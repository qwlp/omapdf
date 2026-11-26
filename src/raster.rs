//! Persistent, statically linked MuPDF worker. Native objects stay on their creating thread;
//! only immutable BGRA images cross into GPUI. No subprocess, PNG, or disk round trip.
use crate::{document::Document, document_theme::DocumentTheme};
use anyhow::{Context, Result};
use gpui::RenderImage;
use parking_lot::{Condvar, Mutex as FastMutex};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering},
        mpsc,
    },
};

#[derive(Clone, PartialEq, Eq)]
struct Key {
    document: String,
    page: usize,
    width: u32,
    dark: bool,
    contrast: u8,
    quality: u16,
    sharpness: u8,
    theme: Option<DocumentTheme>,
}
struct Entry {
    key: Key,
    image: Arc<RenderImage>,
    bytes: usize,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
}
impl Cache {
    fn get(&mut self, key: &Key) -> Option<Arc<RenderImage>> {
        let index = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let image = entry.image.clone();
        self.entries.push_front(entry);
        Some(image)
    }
    fn get_nearest(&self, key: &Key) -> Option<(u32, Arc<RenderImage>)> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.key.document == key.document
                    && entry.key.page == key.page
                    && entry.key.dark == key.dark
                    && entry.key.contrast == key.contrast
                    && entry.key.quality == key.quality
                    && entry.key.sharpness == key.sharpness
                    && entry.key.theme == key.theme
            })
            .min_by_key(|entry| entry.key.width.abs_diff(key.width))
            .map(|entry| (entry.key.width, entry.image.clone()))
    }
    fn insert(&mut self, key: Key, image: Arc<RenderImage>) {
        if self.entries.iter().any(|entry| entry.key == key) {
            return;
        }
        let bytes = image.as_bytes(0).map_or(0, <[u8]>::len);
        self.bytes += bytes;
        self.entries.push_front(Entry { key, image, bytes });
        while self.bytes > 192 * 1024 * 1024 && self.entries.len() > 1 {
            if let Some(entry) = self.entries.pop_back() {
                self.bytes -= entry.bytes;
            }
        }
    }
}
struct Work {
    document: Arc<Document>,
    key: Key,
    reply: Option<mpsc::Sender<Result<Arc<RenderImage>>>>,
}

#[derive(Default)]
struct Queue {
    foreground: VecDeque<Work>,
    prefetch: VecDeque<Work>,
    stop: bool,
}

pub struct Rasterizer {
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    contrast: AtomicU8,
    quality: AtomicU16,
    sharpness: AtomicU8,
    theme: Mutex<Option<DocumentTheme>>,
    queue: Arc<(FastMutex<Queue>, Condvar)>,
    cache: Arc<FastMutex<Cache>>,
}
impl Default for Rasterizer {
    fn default() -> Self {
        Self::new()
    }
}
impl Rasterizer {
    pub fn new() -> Self {
        let queue = Arc::new((FastMutex::new(Queue::default()), Condvar::new()));
        let worker_queue = queue.clone();
        let cache = Arc::new(FastMutex::new(Cache::default()));
        let worker_cache = cache.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("pdf-raster".into())
            .spawn(move || {
                let mut documents: VecDeque<(String, NativeDocument)> = VecDeque::new();
                let mut resizer = fast_image_resize::Resizer::new();
                let (lock, condvar) = &*worker_queue;
                loop {
                    let work = {
                        let mut state = lock.lock();
                        loop {
                            if state.stop || worker_stop.load(Ordering::Acquire) {
                                return;
                            }
                            if let Some(work) = state.foreground.pop_front() {
                                break work;
                            }
                            if let Some(work) = state.prefetch.pop_front() {
                                break work;
                            }
                            condvar.wait(&mut state);
                        }
                    };
                    let result = (|| {
                        if let Some(image) = worker_cache.lock().get(&work.key) {
                            return Ok(image);
                        }
                        let index = documents
                            .iter()
                            .position(|(id, _)| id == &work.key.document);
                        if let Some(index) = index {
                            let entry = documents.remove(index).unwrap();
                            documents.push_front(entry);
                        } else {
                            documents.push_front((
                                work.key.document.clone(),
                                NativeDocument::open(&work.document)?,
                            ));
                            documents.truncate(4);
                        }
                        let page = work
                            .document
                            .pages
                            .get(work.key.page)
                            .context("Page out of range")?;
                        let final_height = (work.key.width as f64 * page.height as f64
                            / page.width as f64)
                            .ceil() as u32;
                        anyhow::ensure!(
                            (64..=8192).contains(&work.key.width)
                                && final_height > 0
                                && final_height as u64 * work.key.width as u64 <= 64_000_000,
                            "Page raster exceeds memory limit"
                        );
                        let max_width = (63_000_000f64 * page.width as f64 / page.height as f64)
                            .sqrt()
                            .floor()
                            .min(8192.) as u32;
                        let sample_width =
                            ((work.key.width as f32 * work.key.quality as f32 / 100.).ceil()
                                as u32)
                                .min(max_width)
                                .max(64);
                        let source =
                            documents[0]
                                .1
                                .render(work.key.page, sample_width, work.key.theme)?;
                        let mut pixels = if source.width() == work.key.width
                            && (work.key.quality == 100 || source.height() == final_height)
                        {
                            source
                        } else {
                            use fast_image_resize::{
                                FilterType, PixelType, ResizeAlg, ResizeOptions, images::Image,
                            };
                            let input = Image::from_vec_u8(
                                source.width(),
                                source.height(),
                                source.into_raw(),
                                PixelType::U8x4,
                            )?;
                            let mut output =
                                Image::new(work.key.width, final_height, PixelType::U8x4);
                            // Pages are painted onto opaque white; alpha multiplication is unnecessary.
                            let options = ResizeOptions::new()
                                .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
                                .use_alpha(false);
                            resizer.resize(&input, &mut output, &options)?;
                            image::RgbaImage::from_raw(
                                work.key.width,
                                final_height,
                                output.into_vec(),
                            )
                            .context("Invalid resized page")?
                        };
                        sharpen(&mut pixels, work.key.sharpness);
                        if work.key.theme.is_none() && work.key.dark {
                            let bytes: &mut [u8] = pixels.as_mut();
                            for pixel in bytes.as_chunks_mut::<4>().0 {
                                for channel in &mut pixel[..3] {
                                    *channel = ((255 - *channel) as u16 * work.key.contrast as u16
                                        / 255) as u8;
                                }
                            }
                        }
                        let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
                        worker_cache.lock().insert(work.key, image.clone());
                        Ok(image)
                    })();
                    if let Some(reply) = work.reply {
                        let _ = reply.send(result);
                    }
                }
            })
            .expect("Cannot start PDF renderer");
        Self {
            stop,
            worker: Mutex::new(Some(worker)),
            contrast: AtomicU8::new(255),
            quality: AtomicU16::new(100),
            sharpness: AtomicU8::new(0),
            theme: Mutex::new(None),
            queue,
            cache,
        }
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        {
            let (lock, condvar) = &*self.queue;
            let mut state = lock.lock();
            state.stop = true;
            state.foreground.clear();
            state.prefetch.clear();
            condvar.notify_all();
        }
        // Native page rendering cannot be interrupted mid-call. The worker owns
        // its documents, cache and stop flag, so let it finish the current call
        // without blocking the UI on tab close or application quit. Dropping the
        // handle detaches it; the stop flag prevents the next queued render.
        drop(self.worker.lock().unwrap().take());
    }
    pub fn set_theme(&self, palette: Option<(u32, u32)>) {
        self.set_palette(palette.map(|(bg, fg)| DocumentTheme::new(bg, fg, fg)));
    }
    pub fn set_palette(&self, palette: Option<DocumentTheme>) {
        *self.theme.lock().unwrap() = palette;
    }
    fn theme(&self) -> Option<DocumentTheme> {
        *self.theme.lock().unwrap()
    }
    pub fn set_sharpness(&self, amount: f32) {
        self.sharpness.store(
            (amount.clamp(0., 1.) * 100.).round() as u8,
            Ordering::Relaxed,
        );
    }
    pub fn set_quality(&self, quality: f32) {
        self.quality.store(
            (quality.clamp(1., 2.5) * 100.).round() as u16,
            Ordering::Relaxed,
        );
    }
    pub fn set_contrast(&self, contrast: f32) {
        self.contrast.store(
            (contrast.clamp(0.1, 1.) * 255.).round() as u8,
            Ordering::Relaxed,
        );
    }
    fn make_key(&self, document: &Document, page: usize, width: u32, dark: bool) -> Key {
        Key {
            document: document.id.clone(),
            page,
            width,
            dark,
            quality: self.quality.load(Ordering::Relaxed),
            sharpness: self.sharpness.load(Ordering::Relaxed),
            theme: self.theme(),
            contrast: if dark {
                self.contrast.load(Ordering::Relaxed)
            } else {
                255
            },
        }
    }
    pub fn cached(
        &self,
        document: &Document,
        page: usize,
        width: u32,
        dark: bool,
    ) -> Option<Arc<RenderImage>> {
        let key = self.make_key(document, page, width, dark);
        self.cache.lock().get(&key)
    }
    pub fn cached_nearest(
        &self,
        document: &Document,
        page: usize,
        width: u32,
        dark: bool,
    ) -> Option<(u32, Arc<RenderImage>)> {
        let key = self.make_key(document, page, width, dark);
        self.cache.lock().get_nearest(&key)
    }
    pub fn render(
        &self,
        document: Arc<Document>,
        page: usize,
        width: u32,
        dark: bool,
    ) -> Result<Arc<RenderImage>> {
        anyhow::ensure!(!self.stop.load(Ordering::Acquire), "PDF renderer is closed");
        let key = self.make_key(&document, page, width, dark);
        if let Some(image) = self.cache.lock().get(&key) {
            return Ok(image);
        }
        let (reply, result) = mpsc::channel();
        {
            let (lock, condvar) = &*self.queue;
            let mut state = lock.lock();
            anyhow::ensure!(!state.stop, "PDF worker stopped");
            state.foreground.push_back(Work {
                document,
                key,
                reply: Some(reply),
            });
            condvar.notify_one();
        }
        result.recv().context("PDF worker stopped")?
    }
    pub fn prefetch(&self, document: Arc<Document>, page: usize, width: u32, dark: bool) {
        if self.stop.load(Ordering::Acquire) || page >= document.pages.len() {
            return;
        }
        let key = self.make_key(&document, page, width, dark);
        if self.cache.lock().get(&key).is_some() {
            return;
        }
        let (lock, condvar) = &*self.queue;
        let mut state = lock.lock();
        if state.stop || state.prefetch.iter().any(|work| work.key == key) {
            return;
        }
        if state.prefetch.len() >= 6 {
            state.prefetch.pop_front();
        }
        state.prefetch.push_back(Work {
            document,
            key,
            reply: None,
        });
        condvar.notify_one();
    }
}

impl Drop for Rasterizer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct NativeDocument {
    options: crate::document::EpubOptions,
    document: mupdf::Document,
    lists: VecDeque<(usize, mupdf::DisplayList)>,
}
impl NativeDocument {
    fn open(document: &Document) -> Result<Self> {
        let access = crate::document::native_access(document.epub_options)?;
        Ok(Self {
            options: document.epub_options,
            document: document.reopen_native(&access)?,
            lists: VecDeque::new(),
        })
    }
    fn render(
        &mut self,
        index: usize,
        width: u32,
        theme: Option<DocumentTheme>,
    ) -> Result<image::RgbaImage> {
        let _access = crate::document::native_access(self.options)?;
        if let Some(position) = self.lists.iter().position(|(page, _)| *page == index) {
            let item = self.lists.remove(position).unwrap();
            self.lists.push_front(item);
        } else {
            let page = self.document.load_page(index as i32)?;
            self.lists.push_front((index, page.to_display_list(true)?));
            self.lists.truncate(12);
        }
        let list = &self.lists[0].1;
        let bounds = list.bounds();
        let scale = width as f32 / (bounds.x1 - bounds.x0);
        let matrix =
            mupdf::Matrix::new(scale, 0., 0., scale, -bounds.x0 * scale, -bounds.y0 * scale);
        let pixmap = if let Some(theme) = theme {
            crate::document_theme::render(list, &matrix, theme)?
        } else {
            // Render directly into an opaque-white 4-channel BGRA pixmap to avoid
            // a per-pixel 3-byte BGR -> 4-byte BGRA expansion copy.
            let mut pixmap = mupdf::Pixmap::new_with_rect(
                &mupdf::Colorspace::device_bgr(),
                bounds.transform(&matrix).round(),
                true,
            )?;
            pixmap.samples_mut().fill(255);
            let target = mupdf::Device::from_pixmap(&pixmap)?;
            list.run(&target, &matrix, mupdf::Rect::INF)?;
            pixmap
        };
        anyhow::ensure!(
            pixmap.n() == 4 && pixmap.width() > 0 && pixmap.height() > 0,
            "Unexpected MuPDF pixel format"
        );
        let row_bytes = pixmap.width() as usize * 4;
        let height = pixmap.height() as usize;
        let stride = pixmap.stride() as usize;
        let bytes = if stride == row_bytes {
            pixmap.samples()[..row_bytes * height].to_vec()
        } else {
            let mut bytes = Vec::with_capacity(row_bytes * height);
            for row in pixmap.samples().chunks(stride).take(height) {
                bytes.extend_from_slice(&row[..row_bytes]);
            }
            bytes
        };
        image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), bytes)
            .context("Invalid raster dimensions")
    }
}

/// Mild four-neighbour unsharp mask. Preserve flat areas and opaque alpha.
fn sharpen(image: &mut image::RgbaImage, amount: u8) {
    if amount == 0 || image.width() < 3 || image.height() < 3 {
        return;
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let stride = width * 4;
    let source = image.as_raw().clone();
    let output: &mut [u8] = image.as_mut();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let i = y * stride + x * 4;
            for channel in 0..3 {
                let i = i + channel;
                let center = source[i] as i32;
                let detail = center * 4
                    - source[i - 4] as i32
                    - source[i + 4] as i32
                    - source[i - stride] as i32
                    - source[i + stride] as i32;
                output[i] = (center + detail * amount as i32 / 400).clamp(0, 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sharpen;
    #[test]
    fn sharpening_preserves_flat_fields_and_strengthens_a_soft_edge() {
        let mut image = image::RgbaImage::from_pixel(5, 5, image::Rgba([180, 180, 180, 255]));
        let original = image.clone();
        sharpen(&mut image, 35);
        assert_eq!(image, original);
        image.put_pixel(2, 2, image::Rgba([80, 80, 80, 255]));
        sharpen(&mut image, 35);
        assert!(image.get_pixel(2, 2)[0] < 80);
        assert_eq!(image.get_pixel(0, 0), original.get_pixel(0, 0));
        assert!(image.pixels().all(|pixel| pixel[3] == 255));
    }
}
