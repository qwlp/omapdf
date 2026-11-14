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
            .overflow_y_scroll()
            .bg(rgb(self.appearance.background))
            .text_color(rgb(self.appearance.foreground))
            .font_family(self.appearance.font.clone())
            .text_size(px(12.))
            .p_4()
            .child(self.title.clone())
            .when_some(self.image.clone(), |view, image| {
                view.child(img(image).w_full())
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
