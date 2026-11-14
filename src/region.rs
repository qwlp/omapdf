//! Native document-region image composition, independent of the desktop compositor.
use crate::{
    document::{Document, Rect},
    raster::Rasterizer,
};
use anyhow::{Context, Result, ensure};
use image::{Rgba, RgbaImage};
use std::{path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct Placement {
    pub page: usize,
    pub bounds: Rect,
    pub size: [f32; 2],
}
#[derive(Clone, Debug)]
pub struct Overlay {
    pub rect: Rect,
    pub color: u32,
}
#[derive(Clone, Debug)]
pub struct Slice {
    pub page: usize,
    pub source: Rect,
    pub target: Rect,
    pub size: [f32; 2],
    pub raster_width: u32,
    pub overlays: Vec<Overlay>,
}
#[derive(Clone, Debug)]
pub struct Region {
    pub width: u32,
    pub height: u32,
    pub background: u32,
    pub slices: Vec<Slice>,
}
fn valid(rect: Rect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|v| v.is_finite())
        && rect.width > 0.
        && rect.height > 0.
}
pub fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    if !valid(a) || !valid(b) {
        return None;
    }
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let rect = Rect {
        x,
        y,
        width: (a.x + a.width).min(b.x + b.width) - x,
        height: (a.y + a.height).min(b.y + b.height) - y,
    };
    valid(rect).then_some(rect)
}
pub fn between(a: [f32; 2], b: [f32; 2]) -> Rect {
    Rect {
        x: a[0].min(b[0]),
        y: a[1].min(b[1]),
        width: (a[0] - b[0]).abs(),
        height: (a[1] - b[1]).abs(),
    }
}
impl Region {
    pub fn select(
        a: [f32; 2],
        b: [f32; 2],
        viewport: Rect,
        density: f32,
        background: u32,
        pages: &[Placement],
    ) -> Result<Self> {
        ensure!(
            density.is_finite() && (0.5..=4.).contains(&density),
            "Invalid image pixel density"
        );
        let selection =
            intersection(between(a, b), viewport).context("Drag a non-empty document area")?;
        let width = (selection.width * density).ceil() as u32;
        let height = (selection.height * density).ceil() as u32;
        ensure!(
            width > 1
                && height > 1
                && width <= 8192
                && height <= 8192
                && u64::from(width) * u64::from(height) <= 16_000_000,
            "Selected image exceeds the 16-megapixel limit"
        );
        let mut slices = Vec::new();
        for page in pages {
            ensure!(
                page.size.iter().all(|v| v.is_finite() && *v > 0.),
                "Invalid document page geometry"
            );
            if let Some(part) = intersection(selection, page.bounds) {
                let scale = [
                    page.size[0] / page.bounds.width,
                    page.size[1] / page.bounds.height,
                ];
                slices.push(Slice {
                    page: page.page,
                    size: page.size,
                    source: Rect {
                        x: (part.x - page.bounds.x) * scale[0],
                        y: (part.y - page.bounds.y) * scale[1],
                        width: part.width * scale[0],
                        height: part.height * scale[1],
                    },
                    target: Rect {
                        x: (part.x - selection.x) * density,
                        y: (part.y - selection.y) * density,
                        width: part.width * density,
                        height: part.height * density,
                    },
                    raster_width: (page.bounds.width * density).round().clamp(64., 8192.) as u32,
                    overlays: Vec::new(),
                });
            }
        }
        ensure!(
            !slices.is_empty(),
            "Selected area does not intersect a document page"
        );
        Ok(Self {
            width,
            height,
            background,
            slices,
        })
    }
    pub fn compose(&self, mut page: impl FnMut(&Slice) -> Result<RgbaImage>) -> Result<RgbaImage> {
        ensure!(
            self.width > 0
                && self.height > 0
                && u64::from(self.width) * u64::from(self.height) <= 16_000_000,
            "Invalid image dimensions"
        );
        let mut output = RgbaImage::from_pixel(
            self.width,
            self.height,
            Rgba([
                (self.background >> 16) as u8,
                (self.background >> 8) as u8,
                self.background as u8,
                255,
            ]),
        );
        for slice in &self.slices {
            let mut source = page(slice)?;
            ensure!(
                source.width() > 0
                    && source.height() > 0
                    && valid(slice.source)
                    && valid(slice.target),
                "Invalid region geometry"
            );
            let sx = source.width() as f32 / slice.size[0];
            let sy = source.height() as f32 / slice.size[1];
            for overlay in &slice.overlays {
                let rect = Rect {
                    x: overlay.rect.x * sx,
                    y: overlay.rect.y * sy,
                    width: overlay.rect.width * sx,
                    height: overlay.rect.height * sy,
                };
                if let Some(rect) = intersection(
                    rect,
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: source.width() as f32,
                        height: source.height() as f32,
                    },
                ) {
                    let alpha = (overlay.color & 255) as u16;
                    for y in rect.y.floor() as u32
                        ..(rect.y + rect.height).ceil().min(source.height() as f32) as u32
                    {
                        for x in rect.x.floor() as u32
                            ..(rect.x + rect.width).ceil().min(source.width() as f32) as u32
                        {
                            let pixel = source.get_pixel_mut(x, y);
                            for (channel, shift) in [24, 16, 8].into_iter().enumerate() {
                                pixel[channel] = ((u16::from(pixel[channel]) * (255 - alpha)
                                    + ((overlay.color >> shift) & 255) as u16 * alpha
                                    + 127)
                                    / 255) as u8;
                            }
                        }
                    }
                }
            }
            let x = (slice.source.x * sx)
                .floor()
                .clamp(0., source.width() as f32) as u32;
            let y = (slice.source.y * sy)
                .floor()
                .clamp(0., source.height() as f32) as u32;
            let right = ((slice.source.x + slice.source.width) * sx)
                .ceil()
                .clamp(x as f32, source.width() as f32) as u32;
            let bottom = ((slice.source.y + slice.source.height) * sy)
                .ceil()
                .clamp(y as f32, source.height() as f32) as u32;
            ensure!(right > x && bottom > y, "Selected area is outside its page");
            let tx = slice.target.x.floor().max(0.) as u32;
            let ty = slice.target.y.floor().max(0.) as u32;
            let tw = ((slice.target.x + slice.target.width)
                .ceil()
                .min(self.width as f32)
                - tx as f32)
                .max(0.) as u32;
            let th = ((slice.target.y + slice.target.height)
                .ceil()
                .min(self.height as f32)
                - ty as f32)
                .max(0.) as u32;
            ensure!(tw > 0 && th > 0, "Invalid image placement");
            let crop = image::imageops::crop_imm(&source, x, y, right - x, bottom - y).to_image();
            let crop = if crop.width() == tw && crop.height() == th {
                crop
            } else {
                image::imageops::resize(&crop, tw, th, image::imageops::FilterType::Lanczos3)
            };
            image::imageops::replace(&mut output, &crop, i64::from(tx), i64::from(ty));
        }
        Ok(output)
    }
    pub fn render(
        &self,
        document: Arc<Document>,
        renderer: &Rasterizer,
        dark: bool,
    ) -> Result<Vec<u8>> {
        let revision = document.revision;
        let image = self.compose(|slice| {
            let rendered =
                renderer.render(document.clone(), slice.page, slice.raster_width, dark)?;
            let mut bytes = rendered.as_bytes(0).context("Empty page raster")?.to_vec();
            for pixel in bytes.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let size = rendered.size(0);
            RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, bytes)
                .context("Invalid native page raster")
        })?;
        ensure!(
            crate::reload::Revision::read(&document.path)? == revision,
            "Document changed while copying the image; select it again"
        );
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png)?;
        Ok(bytes.into_inner())
    }
}
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure!(
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png")),
        "Choose a new .png file"
    );
    ensure!(!path.exists(), "Image destination already exists");
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)?;
    Ok(())
}
