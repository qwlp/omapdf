//! Object-aware MuPDF recoloring. Images are forwarded unchanged; text and
//! neutral vector fills are recolored before MuPDF antialiases their edges.
use anyhow::Result;
use mupdf::{
    BlendMode, ColorParams, Colorspace, Device, DisplayList, Error, Function, Image, Matrix,
    NativeDevice, Path, Pixmap, Rect, Shade, StrokeState, Text,
};
use std::{cell::RefCell, num::NonZero, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentTheme {
    pub background: u32,
    pub foreground: u32,
    pub accent: u32,
}
impl DocumentTheme {
    pub fn new(background: u32, foreground: u32, accent: u32) -> Self {
        Self {
            background,
            foreground,
            accent,
        }
    }
    fn rgb(color: u32) -> [f32; 3] {
        [
            (color >> 16 & 255) as f32 / 255.,
            (color >> 8 & 255) as f32 / 255.,
            (color & 255) as f32 / 255.,
        ]
    }
    fn map(self, rgb: [f32; 3], text: bool) -> Option<[f32; 3]> {
        let min = rgb.iter().copied().fold(f32::INFINITY, f32::min);
        let max = rgb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if max - min > 24. / 255. {
            return text.then(|| Self::rgb(self.accent));
        }
        let light = (rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722).clamp(0., 1.);
        let bg = Self::rgb(self.background);
        let fg = Self::rgb(self.foreground);
        Some(std::array::from_fn(|i| {
            fg[i] * (1. - light) + bg[i] * light
        }))
    }
}

pub fn render(list: &DisplayList, matrix: &Matrix, theme: DocumentTheme) -> Result<Pixmap> {
    let mut pixmap = Pixmap::new_with_rect(
        &Colorspace::device_bgr(),
        list.bounds().transform(matrix).round(),
        true,
    )?;
    let background = [
        theme.background as u8,
        (theme.background >> 8) as u8,
        (theme.background >> 16) as u8,
        255,
    ];
    for pixel in pixmap.samples_mut().as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&background);
    }
    let errors = Rc::new(RefCell::new(None));
    let target = Device::from_pixmap(&pixmap)?;
    let filter = Device::from_native(ThemedDevice {
        target,
        theme,
        errors: errors.clone(),
        mask_depth: 0,
    })?;
    list.run(&filter, matrix, Rect::INF)?;
    drop(filter);
    if let Some(error) = errors.borrow_mut().take() {
        return Err(error.into());
    }
    Ok(pixmap)
}
struct ThemedDevice {
    target: Device,
    theme: DocumentTheme,
    errors: Rc<RefCell<Option<Error>>>,
    mask_depth: usize,
}
impl ThemedDevice {
    fn record(&self, result: std::result::Result<(), Error>) {
        if let Err(error) = result {
            self.errors.borrow_mut().get_or_insert(error);
        }
    }
    fn color(
        &self,
        cs: &Colorspace,
        color: &[f32],
        cp: ColorParams,
        text: bool,
    ) -> Option<[f32; 3]> {
        if self.mask_depth > 0 {
            return None;
        }
        match cs.convert_color(color, &Colorspace::device_rgb(), None, cp) {
            Ok(rgb) => self.theme.map([rgb[0], rgb[1], rgb[2]], text),
            Err(error) => {
                self.errors.borrow_mut().get_or_insert(error);
                None
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
impl NativeDevice for ThemedDevice {
    fn fill_path(
        &mut self,
        path: &Path,
        even_odd: bool,
        ctm: Matrix,
        cs: &Colorspace,
        color: &[f32],
        alpha: f32,
        cp: ColorParams,
    ) {
        if let Some(rgb) = self.color(cs, color, cp, false) {
            self.record(self.target.fill_path(
                path,
                even_odd,
                &ctm,
                &Colorspace::device_rgb(),
                &rgb,
                alpha,
                cp,
            ));
        } else {
            self.record(
                self.target
                    .fill_path(path, even_odd, &ctm, cs, color, alpha, cp),
            );
        }
    }
    fn stroke_path(
        &mut self,
        path: &Path,
        stroke: &StrokeState,
        ctm: Matrix,
        cs: &Colorspace,
        color: &[f32],
        alpha: f32,
        cp: ColorParams,
    ) {
        if let Some(rgb) = self.color(cs, color, cp, false) {
            self.record(self.target.stroke_path(
                path,
                stroke,
                &ctm,
                &Colorspace::device_rgb(),
                &rgb,
                alpha,
                cp,
            ));
        } else {
            self.record(
                self.target
                    .stroke_path(path, stroke, &ctm, cs, color, alpha, cp),
            );
        }
    }
    fn fill_text(
        &mut self,
        text: &Text,
        ctm: Matrix,
        cs: &Colorspace,
        color: &[f32],
        alpha: f32,
        cp: ColorParams,
    ) {
        if let Some(rgb) = self.color(cs, color, cp, true) {
            self.record(self.target.fill_text(
                text,
                &ctm,
                &Colorspace::device_rgb(),
                &rgb,
                alpha,
                cp,
            ));
        } else {
            self.record(self.target.fill_text(text, &ctm, cs, color, alpha, cp));
        }
    }
    fn stroke_text(
        &mut self,
        text: &Text,
        stroke: &StrokeState,
        ctm: Matrix,
        cs: &Colorspace,
        color: &[f32],
        alpha: f32,
        cp: ColorParams,
    ) {
        if let Some(rgb) = self.color(cs, color, cp, true) {
            self.record(self.target.stroke_text(
                text,
                stroke,
                &ctm,
                &Colorspace::device_rgb(),
                &rgb,
                alpha,
                cp,
            ));
        } else {
            self.record(
                self.target
                    .stroke_text(text, stroke, &ctm, cs, color, alpha, cp),
            );
        }
    }
    fn clip_path(&mut self, path: &Path, even_odd: bool, ctm: Matrix, _scissor: Rect) {
        self.record(self.target.clip_path(path, even_odd, &ctm));
    }
    fn clip_stroke_path(&mut self, path: &Path, stroke: &StrokeState, ctm: Matrix, _scissor: Rect) {
        self.record(self.target.clip_stroke_path(path, stroke, &ctm));
    }
    fn clip_text(&mut self, text: &Text, ctm: Matrix, _scissor: Rect) {
        self.record(self.target.clip_text(text, &ctm));
    }
    fn clip_stroke_text(&mut self, text: &Text, stroke: &StrokeState, ctm: Matrix, _scissor: Rect) {
        self.record(self.target.clip_stroke_text(text, stroke, &ctm));
    }
    fn ignore_text(&mut self, text: &Text, ctm: Matrix) {
        self.record(self.target.ignore_text(text, &ctm));
    }
    fn fill_shade(&mut self, shade: &Shade, ctm: Matrix, alpha: f32, cp: ColorParams) {
        self.record(self.target.fill_shade(shade, &ctm, alpha, cp));
    }
    fn fill_image(&mut self, image: &Image, ctm: Matrix, alpha: f32, cp: ColorParams) {
        self.record(self.target.fill_image(image, &ctm, alpha, cp));
    }
    fn fill_image_mask(
        &mut self,
        image: &Image,
        ctm: Matrix,
        cs: &Colorspace,
        color: &[f32],
        alpha: f32,
        cp: ColorParams,
    ) {
        self.record(
            self.target
                .fill_image_mask(image, &ctm, cs, color, alpha, cp),
        );
    }
    fn clip_image_mask(&mut self, image: &Image, ctm: Matrix, _scissor: Rect) {
        self.record(self.target.clip_image_mask(image, &ctm));
    }
    fn pop_clip(&mut self) {
        self.record(self.target.pop_clip());
    }
    fn begin_mask(
        &mut self,
        area: Rect,
        luminosity: bool,
        cs: &Colorspace,
        color: &[f32],
        cp: ColorParams,
    ) {
        self.record(self.target.begin_mask(area, luminosity, cs, color, cp));
        self.mask_depth += 1;
    }
    fn end_mask(&mut self, f: &Function) {
        self.record(self.target.end_mask(Some(f)));
        self.mask_depth = self.mask_depth.saturating_sub(1);
    }
    fn begin_group(
        &mut self,
        area: Rect,
        cs: &Colorspace,
        isolated: bool,
        knockout: bool,
        mode: BlendMode,
        alpha: f32,
    ) {
        self.record(
            self.target
                .begin_group(area, cs, isolated, knockout, mode, alpha),
        );
    }
    fn end_group(&mut self) {
        self.record(self.target.end_group());
    }
    fn begin_tile(
        &mut self,
        area: Rect,
        view: Rect,
        x: f32,
        y: f32,
        ctm: Matrix,
        id: Option<NonZero<i32>>,
        doc: Option<NonZero<i32>>,
    ) -> Option<NonZero<i32>> {
        match self.target.begin_tile(area, view, x, y, &ctm, id, doc) {
            Ok(value) => value,
            Err(error) => {
                self.errors.borrow_mut().get_or_insert(error);
                None
            }
        }
    }
    fn end_tile(&mut self) {
        self.record(self.target.end_tile());
    }
    fn begin_layer(&mut self, name: &str) {
        self.record(self.target.begin_layer(name));
    }
    fn end_layer(&mut self) {
        self.record(self.target.end_layer());
    }
    fn begin_structure(&mut self, standard: mupdf::device::Structure, raw: &str, index: i32) {
        self.record(self.target.begin_structure(standard, raw, index));
    }
    fn end_structure(&mut self) {
        self.record(self.target.end_structure());
    }
    fn begin_metatext(&mut self, meta: mupdf::device::Metatext, text: &str) {
        self.record(self.target.begin_metatext(meta, text));
    }
    fn end_metatext(&mut self) {
        self.record(self.target.end_metatext());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_are_classified_per_object_before_antialiasing() {
        let theme = DocumentTheme::new(0x102030, 0xe0c0a0, 0x9080e0);
        assert_eq!(
            theme.map([1.; 3], false),
            Some(DocumentTheme::rgb(0x102030))
        );
        assert_eq!(theme.map([0.; 3], true), Some(DocumentTheme::rgb(0xe0c0a0)));
        assert_eq!(
            theme.map([0., 0., 1.], true),
            Some(DocumentTheme::rgb(0x9080e0))
        );
        assert_eq!(theme.map([1., 0., 0.], false), None);
    }
}
