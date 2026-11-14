//! Text-line ruler navigation in native extraction order (including columns).
use crate::document::{Document, Rect, Word};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub page: usize,
    pub word: usize,
    pub rect: Rect,
}

pub fn lines(page: usize, words: &[Word]) -> Vec<Mark> {
    let mut result: Vec<Mark> = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let r = word.rect;
        if ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
            || r.width <= 0.
            || r.height <= 0.
        {
            continue;
        }
        if let Some(last) = result.last_mut() {
            let a = last.rect;
            let overlap = (a.y + a.height).min(r.y + r.height) - a.y.max(r.y);
            let gap = (r.x - a.x - a.width).max(a.x - r.x - r.width).max(0.);
            if overlap >= a.height.min(r.height) * 0.5 && gap <= a.height.max(r.height) * 1.8 {
                let x = a.x.min(r.x);
                let y = a.y.min(r.y);
                last.rect = Rect {
                    x,
                    y,
                    width: (a.x + a.width).max(r.x + r.width) - x,
                    height: (a.y + a.height).max(r.y + r.height) - y,
                };
                continue;
            }
        }
        result.push(Mark {
            page,
            word: index,
            rect: r,
        });
    }
    result
}

pub fn nearest(lines: &[Mark], position: [f32; 2]) -> Option<Mark> {
    lines.iter().copied().min_by(|a, b| {
        let distance = |r: Rect| {
            let dx = (r.x - position[0]).max(position[0] - r.x - r.width).max(0.);
            let dy = (r.y - position[1])
                .max(position[1] - r.y - r.height)
                .max(0.);
            dx * dx + dy * dy
        };
        distance(a.rect).total_cmp(&distance(b.rect))
    })
}

/// Run on a worker: load only pages crossed, skip empty pages, clamp at boundaries.
pub fn navigate(
    doc: &Document,
    mark: Option<Mark>,
    seed: (usize, [f32; 2]),
    delta: i64,
) -> Result<Option<Mark>> {
    navigate_until(
        doc,
        mark,
        seed,
        delta,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

pub fn navigate_until(
    doc: &Document,
    mark: Option<Mark>,
    seed: (usize, [f32; 2]),
    delta: i64,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Option<Mark>> {
    let cancelled = || cancelled.load(std::sync::atomic::Ordering::Relaxed);
    if doc.pages.is_empty() || cancelled() {
        return Ok(None);
    }
    let mut page = mark
        .map_or(seed.0, |mark| mark.page)
        .min(doc.pages.len() - 1);
    doc.ensure_page_text(page)?;
    let mut current = lines(page, &doc.pages[page].words);
    let mut selected = mark
        .and_then(|mark| current.iter().position(|line| line.word == mark.word))
        .or_else(|| {
            nearest(&current, seed.1)
                .and_then(|mark| current.iter().position(|line| line.word == mark.word))
        });
    let forward = delta >= 0;
    let mut remaining = delta.unsigned_abs();
    let mut last = selected.map(|index| current[index]);
    loop {
        if cancelled() {
            return Ok(None);
        }
        if let Some(index) = selected {
            let available = if forward {
                current.len() - 1 - index
            } else {
                index
            };
            if remaining <= available as u64 {
                let index = if forward {
                    index + remaining as usize
                } else {
                    index - remaining as usize
                };
                return Ok(Some(current[index]));
            }
            remaining -= available as u64;
            last = Some(current[if forward { current.len() - 1 } else { 0 }]);
        }
        if (forward && page + 1 == doc.pages.len()) || (!forward && page == 0) {
            return Ok(last);
        }
        page = if forward { page + 1 } else { page - 1 };
        doc.ensure_page_text(page)?;
        current = lines(page, &doc.pages[page].words);
        selected = if current.is_empty() {
            None
        } else {
            Some(if forward { 0 } else { current.len() - 1 })
        };
        if selected.is_some() {
            remaining = remaining.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn word(x: f32, y: f32) -> Word {
        Word {
            text: "word".into(),
            rect: Rect {
                x,
                y,
                width: 20.,
                height: 10.,
            },
        }
    }
    #[test]
    fn preserves_column_reading_order_and_combines_words() {
        let result = lines(
            2,
            &[
                word(10., 10.),
                word(35., 10.),
                word(10., 25.),
                word(200., 10.),
                word(225., 10.),
            ],
        );
        assert_eq!(result.len(), 3);
        assert_eq!(
            result.iter().map(|line| line.word).collect::<Vec<_>>(),
            [0, 2, 3]
        );
        assert_eq!(result[0].rect.width, 45.);
        assert_eq!(nearest(&result, [210., 11.]), Some(result[2]));
        assert_eq!(nearest(&result, [11., 29.]), Some(result[1]));
    }
    #[test]
    fn empty_and_invalid_geometry_is_ignored() {
        assert!(nearest(&[], [0., 0.]).is_none());
        assert!(lines(0, &[word(f32::NAN, 0.), word(0., f32::INFINITY)]).is_empty());
    }
}
