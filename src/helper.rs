use gpui::{prelude::*, *};
use std::sync::Arc;

pub struct Helper {
    pub image: Option<Arc<RenderImage>>,
    pub title: String,
    pub appearance: omapdf::appearance::Appearance,
}

impl Render for Helper {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.title);
        div()
            .id("portal-helper")
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_hidden()
            .bg(rgb(self.appearance.background))
            .text_color(rgb(self.appearance.foreground))
            .font_family(self.appearance.font.clone())
            .text_size(px(12.))
            .p_4()
            .child(self.title.clone())
            .when_some(self.image.clone(), |view, image| {
                let image_size = image.size(0);
                let scale = ((f32::from(window.viewport_size().width) - 32.).max(1.)
                    / image_size.width.0.max(1) as f32)
                    .min(
                        (f32::from(window.viewport_size().height) - 64.).max(1.)
                            / image_size.height.0.max(1) as f32,
                    );
                view.child(
                    img(image)
                        .flex_shrink_0()
                        .w(px(image_size.width.0 as f32 * scale))
                        .h(px(image_size.height.0 as f32 * scale)),
                )
            })
    }
}

pub struct Hint {
    pub label: SharedString,
    pub appearance: omapdf::appearance::Appearance,
}
impl Render for Hint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(self.appearance.surface))
            .text_color(rgb(self.appearance.foreground))
            .font_family(self.appearance.font.clone())
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(12.))
            .shadow_md()
            .child(self.label.clone())
    }
}
