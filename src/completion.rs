//! Command-line candidates and argument hints, independent of the editor UI.
use crate::{
    commands::COMMANDS,
    settings::{Kind, SETTINGS},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub value: String,
    pub label: String,
    pub detail: String,
}

pub fn usage(command: &str) -> Option<&'static str> {
    match command {
        "goto" | "goto_page_with_page_number" => Some("PAGE"),
        "export_image" | "open" | "open_document" | "embed_annotations" | "import"
        | "import_sqlite" | "export" => Some("PATH"),
        "search" | "regex_search" => Some("TEXT"),
        "add_bookmark" => Some("NAME"),
        "edit_selected_highlight"
        | "add_annot_to_highlight"
        | "add_annot_to_selected_highlight" => Some("COMMENT"),
        "set_preference" => Some("NAME VALUE"),
        "set_mark" | "goto_mark" => Some("LETTER"),
        "set_status_string" => Some("TEXT"),
        _ => None,
    }
}

pub fn active(input: &str) -> &str {
    input.rsplit(';').next().unwrap_or(input).trim_start()
}

pub fn candidates(input: &str, history: &[String]) -> Vec<Candidate> {
    let tail = active(input);
    let prefix = &input[..input.len() - tail.len()];
    let mut values: Vec<(String, String, String)> = Vec::new();
    if let Some((command, argument)) = tail.split_once(' ') {
        if command == "set_preference" {
            if let Some((key, value)) = argument.trim_start().split_once(' ') {
                if let Some(setting) = SETTINGS.iter().find(|setting| setting.key == key) {
                    let choices: &[&str] = match setting.kind {
                        Kind::Toggle => &["0", "1"],
                        Kind::Layout => &["single", "vertical", "horizontal", "spread"],
                        _ => &[],
                    };
                    for choice in choices
                        .iter()
                        .filter(|choice| choice.starts_with(value.trim()))
                    {
                        values.push((
                            format!("set_preference {key} {choice}"),
                            choice.to_string(),
                            setting.title.into(),
                        ));
                    }
                }
            } else {
                for setting in SETTINGS {
                    if rank(setting.key, argument.trim()).is_some() {
                        values.push((
                            format!("set_preference {}", setting.key),
                            setting.key.into(),
                            setting.title.into(),
                        ));
                    }
                }
                values.sort_by_key(|(_, label, _)| rank(label, argument.trim()));
            }
        }
    } else {
        let mut commands = COMMANDS
            .iter()
            .filter_map(|command| rank(command, tail).map(|rank| (rank, *command)))
            .collect::<Vec<_>>();
        commands.sort_by_key(|(rank, command)| (*rank, *command));
        for (_, command) in commands {
            values.push((
                command.into(),
                command.into(),
                usage(command)
                    .unwrap_or_else(|| crate::commands::description(command))
                    .into(),
            ));
        }
    }
    let mut result = Vec::new();
    if tail.is_empty() && prefix.is_empty() {
        for command in history.iter().rev() {
            if !result
                .iter()
                .any(|candidate: &Candidate| candidate.value == *command)
            {
                result.push(Candidate {
                    value: command.clone(),
                    label: command.clone(),
                    detail: "Recent".into(),
                });
            }
        }
    }
    for (value, label, detail) in values {
        let value = format!("{prefix}{value}");
        if !result.iter().any(|candidate| candidate.value == value) {
            result.push(Candidate {
                value,
                label,
                detail,
            });
        }
    }
    result
}

fn rank(value: &str, query: &str) -> Option<(usize, usize)> {
    let query = query.to_lowercase().replace(' ', "_");
    if value.starts_with(&query) {
        return Some((0, value.len()));
    }
    if let Some(index) = value.find(&query) {
        return Some((1, index));
    }
    let mut chars = value.chars();
    query
        .chars()
        .all(|wanted| chars.any(|ch| ch == wanted))
        .then_some((2, value.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_prefixes_then_substrings_then_fuzzy_matches() {
        let result = candidates("goto_t", &[]);
        assert_eq!(result[0].value, "goto_toc");
        assert!(
            result
                .iter()
                .any(|candidate| candidate.value == "goto_top_of_page")
        );
        assert!(
            candidates("ftpw", &[])
                .iter()
                .any(|candidate| candidate.value == "fit_to_page_width")
        );
        assert!(candidates("你好", &[]).is_empty());
    }
    #[test]
    fn completes_preferences_and_values_without_changing_prior_commands() {
        assert_eq!(
            candidates("goto 12; set_preference wheel_scroll_m", &[])[0].value,
            "goto 12; set_preference wheel_scroll_multiplier"
        );
        let values = candidates("set_preference smooth_scroll ", &[]);
        assert_eq!(
            values
                .iter()
                .map(|candidate| candidate.label.as_str())
                .collect::<Vec<_>>(),
            ["0", "1"]
        );
        assert_eq!(
            candidates("set_preference page_layout sp", &[])[0].value,
            "set_preference page_layout spread"
        );
        assert!(candidates("search arbitrary text", &[]).is_empty());
    }
    #[test]
    fn recent_commands_are_deduplicated_and_arguments_have_hints() {
        let result = candidates("", &["goto 42".into(), "zoom_in".into(), "goto 42".into()]);
        assert_eq!(result[0].value, "goto 42");
        assert_eq!(
            result
                .iter()
                .filter(|candidate| candidate.value == "zoom_in")
                .count(),
            1
        );
        assert_eq!(usage("goto"), Some("PAGE"));
    }
}
