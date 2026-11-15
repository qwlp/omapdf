//! Read Omarchy's active palette and font without changing desktop configuration.
use gpui::{AssetSource, SharedString};
use std::{collections::BTreeMap, sync::OnceLock, time::SystemTime};

#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    pub background: u32,
    pub panel: u32,
    pub surface: u32,
    pub foreground: u32,
    pub muted: u32,
    pub accent: u32,
    pub selection: u32,
    pub border: u32,
    pub font: String,
    font_stamp: Option<SystemTime>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: 0x1a1b26,
            panel: 0x1a1b26,
            surface: 0x24283b,
            foreground: 0xa9b1d6,
            muted: 0x737aa2,
            accent: 0x7aa2f7,
            selection: 0x292e42,
            border: 0x292e42,
            font: "JetBrainsMono NF".into(),
            font_stamp: None,
        }
    }
}
impl Appearance {
    pub fn load() -> Self {
        // Resolve the host font once per process. The main thread starts this
        // during platform initialization; additional tabs only check for changes.
        static INITIAL: OnceLock<Appearance> = OnceLock::new();
        INITIAL
            .get_or_init(|| Self::default().refresh(true))
            .refresh(false)
    }
    pub fn refresh(&self, force_font: bool) -> Self {
        let mut next = self.clone();
        let home = dirs::home_dir().unwrap_or_default();
        let candidates = [
            dirs::state_dir()
                .unwrap_or_else(|| home.join(".local/state"))
                .join("omarchy/current/theme/colors.toml"),
            dirs::config_dir()
                .unwrap_or_else(|| home.join(".config"))
                .join("omarchy/current/theme/colors.toml"),
        ];
        if let Some(text) = candidates
            .iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
        {
            next.apply_colors(&text);
        }
        let stamp = dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("fontconfig/fonts.conf");
        let stamp = std::fs::metadata(stamp).and_then(|m| m.modified()).ok();
        if force_font || stamp != self.font_stamp {
            if let Ok(output) = std::process::Command::new("fc-match")
                // Query the host's fontconfig so Omarchy's selected font also
                // wins when the reader runs with its private portable runtime.
                .env_remove("FONTCONFIG_FILE")
                .env_remove("FONTCONFIG_PATH")
                .env_remove("FONTCONFIG_SYSROOT")
                .args(["monospace", "-f", "%{family}"])
                .output()
                && output.status.success()
            {
                let font = String::from_utf8_lossy(&output.stdout)
                    .split(',')
                    .next()
                    .unwrap_or("JetBrainsMono NF")
                    .trim()
                    .to_string();
                if !font.is_empty() {
                    next.font = font;
                }
            }
            next.font_stamp = stamp;
        }
        next
    }
    pub fn apply_colors(&mut self, text: &str) {
        let colors: BTreeMap<_, _> = text
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                let value = value.trim().trim_matches(['\"', '\'']).strip_prefix('#')?;
                if value.len() != 6 {
                    return None;
                }
                Some((key.trim(), u32::from_str_radix(value, 16).ok()?))
            })
            .collect();
        let color = |key, fallback| colors.get(key).copied().unwrap_or(fallback);
        self.background = color("dark_background", color("background", self.background));
        self.panel = color("background", self.panel);
        self.surface = color("lighter_background", self.surface);
        self.foreground = color("foreground", self.foreground);
        self.muted = color("dark_foreground", color("muted", self.muted));
        self.accent = color("accent", self.accent);
        self.selection = color("selection", self.selection);
        self.border = self.surface;
    }
}

pub struct Icons;
impl AssetSource for Icons {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "icons/logo.png" => Some(include_bytes!("../assets/icons/logo.png")),
            "icons/logo.svg" => Some(include_bytes!("../assets/icons/logo.svg")),
            "icons/bookmark.svg" => Some(include_bytes!("../assets/icons/bookmark.svg")),
            "icons/check.svg" => Some(include_bytes!("../assets/icons/check.svg")),
            "icons/chevron-left.svg" => Some(include_bytes!("../assets/icons/chevron-left.svg")),
            "icons/chevron-right.svg" => Some(include_bytes!("../assets/icons/chevron-right.svg")),
            "icons/command.svg" => Some(include_bytes!("../assets/icons/command.svg")),
            "icons/external-link.svg" => Some(include_bytes!("../assets/icons/external-link.svg")),
            "icons/folder-open.svg" => Some(include_bytes!("../assets/icons/folder-open.svg")),
            "icons/keyboard.svg" => Some(include_bytes!("../assets/icons/keyboard.svg")),
            "icons/minus.svg" => Some(include_bytes!("../assets/icons/minus.svg")),
            "icons/moon.svg" => Some(include_bytes!("../assets/icons/moon.svg")),
            "icons/panel-left.svg" => Some(include_bytes!("../assets/icons/panel-left.svg")),
            "icons/plus.svg" => Some(include_bytes!("../assets/icons/plus.svg")),
            "icons/refresh-cw.svg" => Some(include_bytes!("../assets/icons/refresh-cw.svg")),
            "icons/rotate-ccw.svg" => Some(include_bytes!("../assets/icons/rotate-ccw.svg")),
            "icons/scan.svg" => Some(include_bytes!("../assets/icons/scan.svg")),
            "icons/chevron-down.svg" => Some(include_bytes!("../assets/icons/chevron-down.svg")),
            "icons/case-sensitive.svg" => {
                Some(include_bytes!("../assets/icons/case-sensitive.svg"))
            }
            "icons/whole-word.svg" => Some(include_bytes!("../assets/icons/whole-word.svg")),
            "icons/regex.svg" => Some(include_bytes!("../assets/icons/regex.svg")),
            "icons/book-open.svg" => Some(include_bytes!("../assets/icons/book-open.svg")),
            "icons/chevrons-up.svg" => Some(include_bytes!("../assets/icons/chevrons-up.svg")),
            "icons/chevrons-down.svg" => Some(include_bytes!("../assets/icons/chevrons-down.svg")),
            "icons/list-tree.svg" => Some(include_bytes!("../assets/icons/list-tree.svg")),
            "icons/info.svg" => Some(include_bytes!("../assets/icons/info.svg")),
            "icons/search.svg" => Some(include_bytes!("../assets/icons/search.svg")),
            "icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "icons/sun.svg" => Some(include_bytes!("../assets/icons/sun.svg")),
            "icons/x.svg" => Some(include_bytes!("../assets/icons/x.svg")),
            "icons/file.svg" => Some(include_bytes!("../assets/icons/file.svg")),
            "icons/columns-2.svg" => Some(include_bytes!("../assets/icons/columns-2.svg")),
            "icons/gallery-horizontal.svg" => {
                Some(include_bytes!("../assets/icons/gallery-horizontal.svg"))
            }
            "icons/gallery-vertical.svg" => {
                Some(include_bytes!("../assets/icons/gallery-vertical.svg"))
            }
            "icons/copy.svg" => Some(include_bytes!("../assets/icons/copy.svg")),
            "icons/highlighter.svg" => Some(include_bytes!("../assets/icons/highlighter.svg")),
            "icons/link.svg" => Some(include_bytes!("../assets/icons/link.svg")),
            "icons/corner-down-right.svg" => {
                Some(include_bytes!("../assets/icons/corner-down-right.svg"))
            }
            "icons/trash-2.svg" => Some(include_bytes!("../assets/icons/trash-2.svg")),
            _ => None,
        };
        Ok(bytes.map(std::borrow::Cow::Borrowed))
    }
    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
