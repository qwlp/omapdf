use crate::document::Page;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    Single,
    Spread,
    Horizontal,
    Vertical,
}
impl LayoutMode {
    pub const ALL: [Self; 4] = [Self::Single, Self::Spread, Self::Horizontal, Self::Vertical];
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Spread => "spread",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single page",
            Self::Spread => "Two pages",
            Self::Horizontal => "Horizontal strip",
            Self::Vertical => "Vertical strip",
        }
    }
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.split_whitespace().next().unwrap_or("") {
            "single" | "one_page" => Ok(Self::Single),
            "spread" | "two_page" => Ok(Self::Spread),
            "horizontal" => Ok(Self::Horizontal),
            "vertical" => Ok(Self::Vertical),
            _ => anyhow::bail!("Expected single, spread, horizontal, or vertical"),
        }
    }
    pub fn continuous(self) -> bool {
        matches!(self, Self::Horizontal | Self::Vertical)
    }
    pub fn step(self) -> usize {
        if self == Self::Spread { 2 } else { 1 }
    }
    pub fn next(self) -> Self {
        Self::ALL[(Self::ALL.iter().position(|m| *m == self).unwrap() + 1) % 4]
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PageBox {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug)]
pub struct PageLayout {
    pub mode: LayoutMode,
    pub pages: Vec<PageBox>,
    pub width: f32,
    pub height: f32,
}
impl PageLayout {
    pub fn new(pages: &[Page], mode: LayoutMode, current: usize, width: f32, gap: f32) -> Self {
        let mut layout = Self {
            mode,
            pages: vec![],
            width: 0.,
            height: 0.,
        };
        if pages.is_empty() {
            return layout;
        }
        let current = current.min(pages.len() - 1);
        let indices: Box<dyn Iterator<Item = usize>> = match mode {
            LayoutMode::Single => Box::new(std::iter::once(current)),
            LayoutMode::Spread => Box::new(current / 2 * 2..(current / 2 * 2 + 2).min(pages.len())),
            _ => Box::new(0..pages.len()),
        };
        let strip_height = width * pages[0].height / pages[0].width;
        for index in indices {
            let page = &pages[index];
            let (w, h) = if mode == LayoutMode::Horizontal {
                (strip_height * page.width / page.height, strip_height)
            } else {
                (width, width * page.height / page.width)
            };
            let (x, y) = match mode {
                LayoutMode::Vertical => (0., layout.height),
                LayoutMode::Horizontal => (layout.width, 0.),
                LayoutMode::Spread => (
                    if layout.pages.is_empty() {
                        0.
                    } else {
                        width + gap
                    },
                    0.,
                ),
                LayoutMode::Single => (0., 0.),
            };
            layout.width = layout.width.max(x + w);
            layout.height = layout.height.max(y + h);
            layout.pages.push(PageBox {
                page: index,
                x,
                y,
                width: w,
                height: h,
            });
        }
        layout
    }
    pub fn origin(&self, page: usize) -> [f32; 2] {
        if !self.mode.continuous() {
            return [0., 0.];
        }
        self.pages
            .get(page)
            .filter(|p| p.page == page)
            .or_else(|| self.pages.iter().find(|p| p.page == page))
            .map(|p| [p.x, p.y])
            .unwrap_or_default()
    }
    pub fn visible(&self, x: f32, y: f32, width: f32, height: f32) -> Vec<PageBox> {
        let margin = if self.mode == LayoutMode::Horizontal {
            width * 0.5
        } else {
            height * 0.5
        };
        match self.mode {
            LayoutMode::Vertical => {
                let min_y = y - margin;
                let max_y = y + height + margin;
                let start = self.pages.partition_point(|p| p.y + p.height < min_y);
                self.pages[start..]
                    .iter()
                    .copied()
                    .take_while(|p| p.y <= max_y)
                    .filter(|p| p.x + p.width >= x - margin && p.x <= x + width + margin)
                    .collect()
            }
            LayoutMode::Horizontal => {
                let min_x = x - margin;
                let max_x = x + width + margin;
                let start = self.pages.partition_point(|p| p.x + p.width < min_x);
                self.pages[start..]
                    .iter()
                    .copied()
                    .take_while(|p| p.x <= max_x)
                    .filter(|p| p.y + p.height >= y - margin && p.y <= y + height + margin)
                    .collect()
            }
            LayoutMode::Single | LayoutMode::Spread => self
                .pages
                .iter()
                .copied()
                .filter(|p| {
                    p.x + p.width >= x - margin
                        && p.x <= x + width + margin
                        && p.y + p.height >= y - margin
                        && p.y <= y + height + margin
                })
                .collect(),
        }
    }
    pub fn page_at(&self, position: f32) -> Option<usize> {
        if !self.mode.continuous() || self.pages.is_empty() {
            return None;
        }
        let index = match self.mode {
            LayoutMode::Horizontal => self.pages.partition_point(|p| position >= p.x + p.width),
            LayoutMode::Vertical => self.pages.partition_point(|p| position >= p.y + p.height),
            _ => return None,
        };
        self.pages
            .get(index)
            .or_else(|| self.pages.last())
            .map(|p| p.page)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pages() -> Vec<Page> {
        [(600., 800.), (800., 600.), (600., 1000.)]
            .into_iter()
            .map(|(width, height)| Page {
                width,
                height,
                words: vec![].into(),
                links: vec![].into(),
            })
            .collect()
    }
    #[test]
    fn spread_pairs_and_odd_last_page() {
        let pages = pages();
        let first = PageLayout::new(&pages, LayoutMode::Spread, 1, 300., 16.);
        assert_eq!(
            first.pages.iter().map(|p| p.page).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(first.width, 616.);
        let last = PageLayout::new(&pages, LayoutMode::Spread, 2, 300., 16.);
        assert_eq!(last.pages.len(), 1);
        assert_eq!(last.pages[0].page, 2);
    }
    #[test]
    fn strips_are_gapless_and_handle_mixed_page_sizes() {
        let p = pages();
        let vertical = PageLayout::new(&p, LayoutMode::Vertical, 0, 600., 16.);
        assert_eq!(vertical.pages[1].y, 800.);
        assert_eq!(vertical.pages[2].y, 1250.);
        assert_eq!(vertical.height, 2250.);
        let horizontal = PageLayout::new(&p, LayoutMode::Horizontal, 0, 600., 16.);
        assert_eq!(horizontal.pages[1].x, 600.);
        assert_eq!(horizontal.pages[1].height, 800.);
        assert!((horizontal.pages[2].x - 1666.6666).abs() < 0.01);
        assert_eq!(horizontal.page_at(700.), Some(1));
        assert_eq!(vertical.page_at(900.), Some(1));
        assert_eq!(vertical.origin(2), [0., 1250.]);
    }
    #[test]
    fn offscreen_pages_are_not_materialized() {
        let page = Page {
            width: 600.,
            height: 800.,
            words: vec![].into(),
            links: vec![].into(),
        };
        let pages = vec![page; 1000];
        let layout = PageLayout::new(&pages, LayoutMode::Vertical, 0, 600., 0.);
        let visible = layout.visible(0., 400000., 700., 900.);
        assert!(visible.len() < 5);
        assert!(visible.iter().any(|p| p.page == 500));
    }
}
