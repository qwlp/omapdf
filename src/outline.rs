//! Outline visibility preserves document hierarchy while filtering and folding sections.
use crate::document::Outline;
use std::collections::BTreeSet;

/// Only branches can be folded; leaves remain visible when a parent is opened.
pub fn collapsed_branches(entries: &[Outline]) -> BTreeSet<usize> {
    entries
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| (pair[1].depth > pair[0].depth).then_some(index))
        .collect()
}

pub fn visible(
    entries: &[Outline],
    collapsed: &BTreeSet<usize>,
    filter: &str,
    flat: bool,
) -> Vec<usize> {
    let filter = filter.trim().to_lowercase();
    let mut ancestors: Vec<usize> = vec![];
    let mut included = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        while ancestors
            .last()
            .is_some_and(|&i| entries[i].depth >= entry.depth)
        {
            ancestors.pop();
        }
        if !filter.is_empty() {
            let mut terms = filter.chars();
            let mut next = terms.next();
            for ch in entry.title.to_lowercase().chars() {
                if next == Some(ch) {
                    next = terms.next();
                }
            }
            if next.is_none() {
                included.insert(index);
                included.extend(ancestors.iter().copied());
            }
        } else if flat || ancestors.iter().all(|i| !collapsed.contains(i)) {
            included.insert(index);
        }
        ancestors.push(index);
    }
    included.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folding_and_filtering_preserve_nested_heading_context() {
        let entries: Vec<_> = [
            ("Part one", 0),
            ("Chapter", 1),
            ("Routing", 3),
            ("Switching", 3),
            ("Part two", 0),
            ("Appendix", 1),
        ]
        .into_iter()
        .map(|(title, depth)| Outline {
            title: title.into(),
            depth,
            page: 0,
            position: None,
        })
        .collect();
        let collapsed = BTreeSet::from([0]);
        assert_eq!(visible(&entries, &collapsed, "", false), vec![0, 4, 5]);
        assert_eq!(visible(&entries, &collapsed, "rtng", false), vec![0, 1, 2]);
        assert!(visible(&entries, &collapsed, "absent", false).is_empty());
        assert_eq!(
            visible(&entries, &collapsed, "", true),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            visible(&entries, &BTreeSet::from([1]), "", false),
            vec![0, 1, 4, 5]
        );
    }
    #[test]
    fn default_folds_every_branch_and_opening_one_does_not_open_descendants() {
        let entries: Vec<_> = [
            ("Book", 0),
            ("Chapter one", 1),
            ("Section", 2),
            ("Detail", 3),
            ("Chapter two", 1),
            ("Section two", 2),
        ]
        .into_iter()
        .map(|(title, depth)| Outline {
            title: title.into(),
            depth,
            page: 0,
            position: None,
        })
        .collect();
        let mut folded = collapsed_branches(&entries);
        assert_eq!(folded, BTreeSet::from([0, 1, 2, 4]));
        assert_eq!(visible(&entries, &folded, "", false), vec![0]);
        folded.remove(&0);
        assert_eq!(visible(&entries, &folded, "", false), vec![0, 1, 4]);
        assert_eq!(
            visible(&entries, &folded, "detail", false),
            vec![0, 1, 2, 3]
        );
        assert_eq!(
            visible(&entries, &BTreeSet::new(), "", false),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert!(collapsed_branches(&[]).is_empty());
    }
}
