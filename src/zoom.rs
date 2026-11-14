//! Preserve a document point under the pointer across layout and zoom changes.
use crate::layout::PageLayout;
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub page: usize,
    fraction: [f32; 2],
    viewport_point: [f32; 2],
}
impl Anchor {
    pub fn capture(
        layout: &PageLayout,
        viewport_point: [f32; 2],
        offset: [f32; 2],
        inset: [f32; 2],
        current: usize,
    ) -> Option<Self> {
        let content = [
            viewport_point[0] - offset[0] - inset[0],
            viewport_point[1] - offset[1] - inset[1],
        ];
        let page = layout
            .pages
            .iter()
            .find(|page| {
                content[0] >= page.x
                    && content[0] <= page.x + page.width
                    && content[1] >= page.y
                    && content[1] <= page.y + page.height
            })
            .or_else(|| layout.pages.iter().find(|page| page.page == current))?;
        Some(Self {
            page: page.page,
            fraction: [
                (content[0] - page.x) / page.width,
                (content[1] - page.y) / page.height,
            ],
            viewport_point,
        })
    }
    pub fn offset(
        self,
        layout: &PageLayout,
        inset: [f32; 2],
        viewport: [f32; 2],
        content: [f32; 2],
    ) -> Option<[f32; 2]> {
        let page = layout.pages.iter().find(|page| page.page == self.page)?;
        Some([
            (self.viewport_point[0] - inset[0] - page.x - self.fraction[0] * page.width)
                .clamp(-(content[0] - viewport[0]).max(0.), 0.),
            (self.viewport_point[1] - inset[1] - page.y - self.fraction[1] * page.height)
                .clamp(-(content[1] - viewport[1]).max(0.), 0.),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::Page, layout::LayoutMode};
    #[test]
    fn point_stays_under_cursor_in_every_layout_and_clamps_at_edges() {
        let pages = (0..5)
            .map(|_| Page {
                width: 600.,
                height: 900.,
                words: vec![].into(),
                links: vec![].into(),
            })
            .collect::<Vec<_>>();
        for mode in LayoutMode::ALL {
            let old = PageLayout::new(&pages, mode, 0, 600., 16.);
            let new = PageLayout::new(&pages, mode, 0, 1200., 16.);
            let pointer = [300., 400.];
            let anchor = Anchor::capture(&old, pointer, [0., -100.], [16., 16.], 0).unwrap();
            let content = [new.width + 32., new.height + 32.];
            let offset = anchor
                .offset(&new, [16., 16.], [640., 800.], content)
                .unwrap();
            let page = &new.pages[anchor.page];
            assert!(
                (16. + page.x + anchor.fraction[0] * page.width + offset[0] - pointer[0]).abs()
                    < 0.01
            );
            assert!(
                (16. + page.y + anchor.fraction[1] * page.height + offset[1] - pointer[1]).abs()
                    < 0.01
            );
            let small = PageLayout::new(&pages, LayoutMode::Single, 0, 200., 16.);
            assert_eq!(
                anchor.offset(&small, [220., 16.], [640., 800.], [640., 800.]),
                Some([0., 0.])
            );
        }
    }
}
