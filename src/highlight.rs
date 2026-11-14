//! Turn word rectangles into continuous line highlights without bridging columns.
use crate::document::Rect;

pub fn line_boxes(words: &[Rect]) -> Vec<Rect> {
    let mut words: Vec<_> = words
        .iter()
        .copied()
        .filter(|r| {
            r.x.is_finite()
                && r.y.is_finite()
                && r.width.is_finite()
                && r.height.is_finite()
                && r.width > 0.
                && r.height > 0.
        })
        .collect();
    words.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<Rect> = vec![];
    for word in words {
        if let Some(line) = lines.iter_mut().rev().find(|line| {
            let overlap = (line.y + line.height).min(word.y + word.height) - line.y.max(word.y);
            let gap = (word.x - (line.x + line.width))
                .max(line.x - (word.x + word.width))
                .max(0.);
            overlap >= line.height.min(word.height) * 0.5
                && gap <= line.height.max(word.height) * 1.8
        }) {
            let right = (line.x + line.width).max(word.x + word.width);
            let bottom = (line.y + line.height).max(word.y + word.height);
            line.x = line.x.min(word.x);
            line.y = line.y.min(word.y);
            line.width = right - line.x;
            line.height = bottom - line.y;
        } else {
            lines.push(word);
        }
    }
    lines
}

/// Legacy uppercase types underline; `_` strikes through text.
pub fn decoration(kind: char, mut rect: Rect) -> Rect {
    if kind.is_ascii_uppercase() || kind == '_' {
        let thickness = (rect.height * 0.075).max(0.7);
        rect.y += if kind == '_' {
            rect.height * 0.5
        } else {
            rect.height - thickness
        };
        rect.height = thickness;
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_word_spaces_but_keeps_lines_and_columns_separate() {
        let word = |x, y, width| Rect {
            x,
            y,
            width,
            height: 10.,
        };
        let boxes = line_boxes(&[
            word(10., 10., 20.),
            word(35., 10., 25.),
            word(10., 24., 30.),
            word(200., 10., 30.),
        ]);
        assert_eq!(boxes.len(), 3);
        assert!(boxes.contains(&word(10., 10., 50.)));
        assert!(boxes.contains(&word(10., 24., 30.)));
        assert!(boxes.contains(&word(200., 10., 30.)));
        assert!(line_boxes(&[]).is_empty());
    }
}
