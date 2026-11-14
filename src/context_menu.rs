#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Copy,
    SelectRegion,
    CopyImage,
    ExportImage,
    Find,
    Highlight,
    Bookmark,
    OpenLink,
    CopyLink,
    JumpReference,
    PreviewReference,
    DeleteHighlight,
    EditHighlightNote,
    Previous,
    Next,
    FitWidth,
    FitPage,
    ToggleDark,
    Settings,
}
#[derive(Clone, Debug)]
pub struct MenuState {
    pub x: f32,
    pub y: f32,
    pub page: usize,
    pub word: Option<usize>,
    pub link: Option<String>,
    pub reference: Option<(usize, usize)>,
    pub highlight: Option<usize>,
    pub active: usize,
}
#[derive(Clone)]
pub enum Item {
    Action {
        action: Action,
        label: &'static str,
        icon: &'static str,
        hint: &'static str,
        enabled: bool,
    },
    Separator,
    Colors,
    Layouts,
}
impl MenuState {
    pub fn items(&self, selected: bool, previous: bool, next: bool, dark: bool) -> Vec<Item> {
        let mut items = vec![];
        let mut action = |action, label, icon, hint, enabled| {
            items.push(Item::Action {
                action,
                label,
                icon,
                hint,
                enabled,
            })
        };
        action(Action::Copy, "Copy selection", "copy", "Ctrl+C", selected);
        if selected {
            action(Action::Find, "Find selected text", "search", "/", true);
            action(
                Action::Highlight,
                "Highlight selection",
                "highlighter",
                "h",
                true,
            );
            items.push(Item::Colors);
        }
        if self.link.is_some() {
            items.push(Item::Separator);
            items.push(Item::Action {
                action: Action::OpenLink,
                label: "Open link",
                icon: "external-link",
                hint: "",
                enabled: true,
            });
            items.push(Item::Action {
                action: Action::CopyLink,
                label: "Copy link address",
                icon: "link",
                hint: "",
                enabled: true,
            });
        }
        if self.reference.is_some() {
            items.push(Item::Separator);
            items.push(Item::Action {
                action: Action::JumpReference,
                label: "Jump to reference",
                icon: "corner-down-right",
                hint: "",
                enabled: true,
            });
            items.push(Item::Action {
                action: Action::PreviewReference,
                label: "Preview reference",
                icon: "panel-left",
                hint: "",
                enabled: true,
            });
        }
        if self.highlight.is_some() {
            items.push(Item::Action {
                action: Action::EditHighlightNote,
                label: "Edit highlight note",
                icon: "bookmark",
                hint: "",
                enabled: true,
            });
            items.push(Item::Action {
                action: Action::DeleteHighlight,
                label: "Remove highlight",
                icon: "trash-2",
                hint: "",
                enabled: true,
            });
        }
        items.push(Item::Separator);
        items.push(Item::Action {
            action: Action::Bookmark,
            label: if selected {
                "Bookmark selection"
            } else {
                "Bookmark this page"
            },
            icon: "bookmark",
            hint: "b",
            enabled: true,
        });
        items.push(Item::Action {
            action: Action::Previous,
            label: "Previous page",
            icon: "chevron-left",
            hint: "",
            enabled: previous,
        });
        items.push(Item::Action {
            action: Action::Next,
            label: "Next page",
            icon: "chevron-right",
            hint: "",
            enabled: next,
        });
        items.push(Item::Separator);
        items.push(Item::Action {
            action: Action::FitWidth,
            label: "Fit to width",
            icon: "scan",
            hint: "w",
            enabled: true,
        });
        items.push(Item::Action {
            action: Action::FitPage,
            label: "Fit page",
            icon: "file",
            hint: "f",
            enabled: true,
        });
        items.push(Item::Layouts);
        items.push(Item::Action {
            action: Action::ToggleDark,
            label: if dark { "Light pages" } else { "Dark pages" },
            icon: if dark { "sun" } else { "moon" },
            hint: "F8",
            enabled: true,
        });
        items.push(Item::Separator);
        items.push(Item::Action {
            action: Action::Settings,
            label: "Settings",
            icon: "settings",
            hint: "Ctrl+,",
            enabled: true,
        });
        items
    }
    pub fn position(&self, viewport: [f32; 2], height: f32) -> [f32; 2] {
        [
            self.x.clamp(4., (viewport[0] - 260. - 4.).max(4.)),
            self.y.clamp(
                4.,
                (viewport[1] - height.min(viewport[1] - 8.) - 4.).max(4.),
            ),
        ]
    }
    pub fn advance(&mut self, items: &[Item], forward: bool) {
        for _ in 0..items.len() {
            self.active = if forward {
                (self.active + 1) % items.len()
            } else {
                (self.active + items.len() - 1) % items.len()
            };
            if matches!(items[self.active], Item::Action { enabled: true, .. }) {
                break;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn menu() -> MenuState {
        MenuState {
            x: 900.,
            y: 700.,
            page: 0,
            word: None,
            link: None,
            reference: None,
            highlight: None,
            active: 0,
        }
    }
    #[test]
    fn clamps_to_window_and_skips_disabled_rows() {
        let mut menu = menu();
        assert_eq!(menu.position([1000., 800.], 500.), [736., 296.]);
        let items = menu.items(false, false, true, false);
        menu.advance(&items, true);
        assert!(matches!(
            items[menu.active],
            Item::Action { enabled: true, .. }
        ));
        assert!(!items.iter().any(|item| matches!(item, Item::Colors)));
    }
    #[test]
    fn link_reference_and_selection_actions_are_contextual() {
        let mut menu = menu();
        menu.link = Some("https://example.test".into());
        menu.reference = Some((2, 4));
        let items = menu.items(true, true, true, true);
        for wanted in [
            Action::Copy,
            Action::Find,
            Action::OpenLink,
            Action::JumpReference,
        ] {
            assert!(items.iter().any(
                |item| matches!(item,Item::Action{action,enabled:true,..} if *action==wanted)
            ));
        }
        assert!(items.iter().any(|item| matches!(item, Item::Colors)));
    }
}
