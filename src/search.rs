//! PDF text search with Unicode-aware matching and word-coordinate results.
use crate::document::Document;
use anyhow::Result;
use regex::{Regex, RegexBuilder};
#[derive(Clone, Copy, Default)]
pub struct Options {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}
pub type Hit = (usize, usize, usize);
pub fn pattern(query: &str, options: Options) -> Result<Option<Regex>> {
    if query.trim().is_empty() {
        return Ok(None);
    }
    let text = if options.regex {
        query.to_owned()
    } else {
        query
            .split_whitespace()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join(r"\s+")
    };
    let text = if options.whole_word {
        format!(r"\b(?:{text})\b")
    } else {
        text
    };
    Ok(Some(
        RegexBuilder::new(&text)
            .case_insensitive(!options.case_sensitive)
            .size_limit(8 * 1024 * 1024)
            .build()?,
    ))
}
pub fn find(document: &Document, query: &str, options: Options) -> Result<Vec<Hit>> {
    let Some(pattern) = pattern(query, options)? else {
        return Ok(vec![]);
    };
    document.ensure_all_text()?;
    Ok(document
        .pages
        .iter()
        .enumerate()
        .flat_map(|(page, content)| {
            content
                .find_regex(&pattern)
                .into_iter()
                .map(move |(start, end)| (page, start, end))
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_phrase_case_unicode_and_regex_options() {
        let defaults = Options::default();
        assert!(
            pattern("résumé", defaults)
                .unwrap()
                .unwrap()
                .is_match("RÉSUMÉ")
        );
        assert!(pattern("a.b", defaults).unwrap().unwrap().is_match("a.b"));
        assert!(!pattern("a.b", defaults).unwrap().unwrap().is_match("axb"));
        assert!(
            pattern("hello  research", defaults)
                .unwrap()
                .unwrap()
                .is_match("hello research")
        );
        let whole = Options {
            whole_word: true,
            ..defaults
        };
        assert!(
            !pattern("cat", whole)
                .unwrap()
                .unwrap()
                .is_match("concatenate")
        );
        assert!(pattern("cat", whole).unwrap().unwrap().is_match("a cat."));
        assert!(
            !pattern(
                "Word",
                Options {
                    case_sensitive: true,
                    ..defaults
                }
            )
            .unwrap()
            .unwrap()
            .is_match("word")
        );
        assert!(
            pattern(
                "[",
                Options {
                    regex: true,
                    ..defaults
                }
            )
            .is_err()
        );
        assert!(pattern(" ", defaults).unwrap().is_none());
    }
}
