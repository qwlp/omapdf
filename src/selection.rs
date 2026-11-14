//! Page-aware text selections and logical highlights across lazy native pages.
use crate::{
    document::{Document, Rect},
    state::{AnnotationMeta, Highlight},
};
use anyhow::{Result, ensure};
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint {
    pub page: usize,
    pub word: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub anchor: Endpoint,
    pub focus: Endpoint,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Fragment {
    pub page: usize,
    pub text: String,
    pub rects: Vec<Rect>,
}
impl Span {
    pub fn endpoints(self) -> (Endpoint, Endpoint) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
    pub fn range(self, page: usize, count: usize) -> Option<Range<usize>> {
        let (first, last) = self.endpoints();
        if page < first.page || page > last.page || count == 0 {
            return None;
        }
        let start = if page == first.page {
            first.word.min(count)
        } else {
            0
        };
        let end = if page == last.page {
            last.word.saturating_add(1).min(count)
        } else {
            count
        };
        (start < end).then_some(start..end)
    }
    pub fn cached(self, doc: &Document) -> Option<Vec<Fragment>> {
        let (first, last) = self.endpoints();
        let pages = doc.pages.get(first.page..=last.page)?;
        if pages.iter().any(|page| !page.words.ready()) {
            return None;
        }
        if first.word >= pages.first()?.words.len() || last.word >= pages.last()?.words.len() {
            return None;
        }
        Some(
            pages
                .iter()
                .enumerate()
                .filter_map(|(offset, page)| {
                    let index = first.page + offset;
                    let range = self.range(index, page.words.len())?;
                    let words = &page.words[range];
                    Some(Fragment {
                        page: index,
                        text: words
                            .iter()
                            .map(|word| word.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                        rects: words.iter().map(|word| word.rect).collect(),
                    })
                })
                .collect(),
        )
    }
    pub fn extract(self, doc: &Document, cancel: &AtomicBool) -> Result<Vec<Fragment>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Selection cancelled");
        if let Some(fragments) = self.cached(doc) {
            return Ok(fragments);
        }
        let (first, last) = self.endpoints();
        ensure!(
            last.page < doc.pages.len(),
            "Selection page is outside the document"
        );
        ensure!(
            crate::reload::Revision::read(&doc.path)? == doc.revision,
            "Document changed before selected text loaded; select it again"
        );
        for page in first.page..=last.page {
            ensure!(!cancel.load(Ordering::Relaxed), "Selection cancelled");
            doc.ensure_page_text(page)?;
        }
        ensure!(
            crate::reload::Revision::read(&doc.path)? == doc.revision,
            "Document changed while selected text loaded; select it again"
        );
        self.cached(doc)
            .ok_or_else(|| anyhow::anyhow!("Selection endpoints are outside the page text"))
    }
}
pub fn text(fragments: &[Fragment]) -> String {
    let mut text = String::new();
    let mut previous = None;
    for fragment in fragments {
        if let Some(page) = previous {
            for _ in page..fragment.page {
                text.push('\n');
            }
        }
        text.push_str(&fragment.text);
        previous = Some(fragment.page);
    }
    text
}
pub fn highlights(fragments: Vec<Fragment>, kind: char) -> Vec<Highlight> {
    let parent = AnnotationMeta::new();
    let grouped = fragments.len() > 1;
    fragments
        .into_iter()
        .map(|fragment| {
            let mut meta = parent.clone();
            if grouped {
                meta.group = Some(parent.uuid.clone());
                meta.uuid =
                    AnnotationMeta::stable(&parent.uuid, &serde_json::json!(fragment.page)).uuid;
            }
            Highlight {
                meta,
                page: fragment.page,
                text: fragment.text,
                rects: fragment.rects,
                kind,
                note: None,
            }
        })
        .collect()
}
