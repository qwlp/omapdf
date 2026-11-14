use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// UTF-8 storage with UTF-16 ranges at the platform input boundary.
#[derive(Default, Debug)]
pub struct TextBuffer {
    pub text: String,
    pub selection: Range<usize>,
    pub marked: Option<Range<usize>>,
}

impl TextBuffer {
    pub fn from_utf16(&self, range: Range<usize>) -> Range<usize> {
        utf16_offset(&self.text, range.start)..utf16_offset(&self.text, range.end)
    }
    pub fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text[..range.start].encode_utf16().count()
            ..self.text[..range.end].encode_utf16().count()
    }
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str, marked: bool) -> usize {
        let range = range
            .map(|r| self.from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let text = text.replace(['\n', '\r'], " ");
        self.text.replace_range(range.clone(), &text);
        let end = range.start + text.len();
        self.selection = end..end;
        self.marked = (marked && !text.is_empty()).then_some(range.start..end);
        range.start
    }
    pub fn backspace(&mut self) {
        if self.selection.is_empty() {
            self.selection.start = self.previous_boundary(self.selection.start);
        }
        self.replace(None, "", false);
    }
    pub fn delete(&mut self) {
        if self.selection.is_empty() {
            self.selection.end = self.next_boundary(self.selection.end);
        }
        self.replace(None, "", false);
    }
    pub fn move_cursor(&mut self, right: bool) {
        let offset = if !self.selection.is_empty() {
            if right {
                self.selection.end
            } else {
                self.selection.start
            }
        } else if right {
            self.next_boundary(self.selection.end)
        } else {
            self.previous_boundary(self.selection.start)
        };
        self.selection = offset..offset;
        self.marked = None;
    }
    fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .take_while(|index| *index < offset)
            .last()
            .unwrap_or(0)
    }
    fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .find(|index| *index > offset)
            .unwrap_or(self.text.len())
    }
}

fn utf16_offset(text: &str, target: usize) -> usize {
    let mut offset = 0;
    for (byte, character) in text.char_indices() {
        if offset >= target {
            return byte;
        }
        offset += character.len_utf16();
        if offset > target {
            return byte;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_deletion_keep_extended_graphemes_whole() {
        for cluster in ["e\u{301}", "👩🏽‍💻", "🇹🇭", "ก้"] {
            let mut input = TextBuffer::default();
            input.replace(None, &format!("a{cluster}z"), false);
            input.move_cursor(false);
            input.move_cursor(false);
            assert_eq!(input.selection, 1..1);
            input.move_cursor(true);
            assert_eq!(input.selection, 1 + cluster.len()..1 + cluster.len());
            input.backspace();
            assert_eq!(input.text, "az");
            input.replace(None, cluster, false);
            input.move_cursor(false);
            input.delete();
            assert_eq!(input.text, "az");
        }
    }
    #[test]
    fn arrows_collapse_selection_to_its_edges() {
        let mut input = TextBuffer::default();
        input.replace(None, "abcd", false);
        input.selection = 1..3;
        input.move_cursor(false);
        assert_eq!(input.selection, 1..1);
        input.selection = 1..3;
        input.move_cursor(true);
        assert_eq!(input.selection, 3..3);
    }
}
