use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub layout: crate::layout::LayoutMode,
    pub epub: crate::document::EpubOptions,
    pub bindings: BTreeMap<Vec<String>, String>,
    pub values: BTreeMap<String, String>,
    pub directory: Option<PathBuf>,
    pub dark: bool,
    pub move_amount: f32,
    pub horizontal_move: f32,
    pub screen_ratio: f32,
    pub zoom_factor: f32,
    pub smooth_scroll: bool,
    pub scroll_response: f32,
    pub wheel_multiplier: f32,
    pub render_scale: f32,
    pub render_sharpness: f32,
    pub animations: bool,
    pub rounded_corners: bool,
    pub document_theme: bool,
    pub dark_contrast: f32,
    pub font_size: f32,
    pub page_padding: f32,
    pub max_page_width: f32,
    pub flat_toc: bool,
    pub sort_bookmarks: bool,
    pub restore_position: bool,
    pub statusbar: bool,
    pub highlights: bool,
    pub selection_color: u32,
    pub search_color: u32,
    pub highlight_colors: [u32; 26],
}

#[derive(Debug, PartialEq)]
pub enum Binding<'a> {
    Exact(&'a str),
    Prefix,
    Missing,
}

impl Default for Config {
    fn default() -> Self {
        let mut config = Self {
            layout: crate::layout::LayoutMode::Single,
            epub: crate::document::EpubOptions::default(),
            bindings: BTreeMap::new(),
            values: BTreeMap::new(),
            directory: None,
            dark: false,
            move_amount: 60.,
            horizontal_move: 60.,
            screen_ratio: 0.5,
            zoom_factor: 1.2,
            smooth_scroll: true,
            scroll_response: 0.16,
            wheel_multiplier: 2.,
            render_scale: 1.0,
            render_sharpness: 0.,
            animations: true,
            rounded_corners: true,
            document_theme: false,
            dark_contrast: 0.8,
            font_size: 13.,
            page_padding: 16.,
            max_page_width: 1200.,
            flat_toc: false,
            sort_bookmarks: true,
            restore_position: true,
            statusbar: true,
            highlights: true,
            selection_color: 0xffff0066,
            search_color: 0x00ff0066,
            highlight_colors: [0xffd54f66; 26],
        };
        config
            .parse_keys(include_str!("../assets/keys.config"))
            .expect("Valid bundled keys");
        config
            .parse_preferences(include_str!("../assets/prefs.config"))
            .expect("Valid Omapdf defaults");

        config
    }
}
impl Config {
    pub fn load(directory: &Path) -> Result<Self> {
        let mut config = Self {
            directory: Some(directory.into()),
            ..Self::default()
        };
        for name in ["keys.config", "keys_user.config"] {
            if let Some(text) = optional_read(&directory.join(name))? {
                config.parse_keys(&text).with_context(|| name.to_string())?;
            }
        }
        for name in ["prefs.config", "prefs_user.config"] {
            if let Some(text) = optional_read(&directory.join(name))? {
                config
                    .parse_preferences(&text)
                    .with_context(|| name.to_string())?;
            }
        }
        Ok(config)
    }
    pub fn parse_preferences(&mut self, text: &str) -> Result<()> {
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(char::is_whitespace) else {
                bail!("Line {}: expected preference and value", index + 1);
            };
            self.apply_preference(key, value.trim())
                .with_context(|| format!("Line {}: {key}", index + 1))?;
        }
        Ok(())
    }
    pub fn apply_preference(&mut self, key: &str, value: &str) -> Result<()> {
        let numeric = |min: f32, max: f32| -> Result<f32> {
            let number: f32 = value
                .split_whitespace()
                .next()
                .context("Missing value")?
                .parse()?;
            anyhow::ensure!(
                number.is_finite() && number >= min && number <= max,
                "Expected a number between {min} and {max}"
            );
            Ok(number)
        };
        let boolean = || -> Result<bool> {
            match value.split_whitespace().next() {
                Some("0") => Ok(false),
                Some("1") => Ok(true),
                _ => bail!("Expected 0 or 1"),
            }
        };
        match key {
            "epub_font_size" => self.epub.font_size = numeric(8., 28.)?,
            "epub_line_height" => self.epub.line_height = numeric(1., 2.)?,
            "epub_page_width" => self.epub.width = numeric(240., 900.)?,
            "epub_page_height" => self.epub.height = numeric(320., 1200.)?,
            "page_layout" => self.layout = crate::layout::LayoutMode::parse(value)?,
            "dark_mode" | "default_dark_mode" => self.dark = boolean()?,
            "move_amount" => self.move_amount = numeric(1., 1000.)?,
            "vertical_move_amount" => self.move_amount = numeric(0.05, 20.)? * 60.,
            "horizontal_move_amount" => self.horizontal_move = numeric(0.05, 20.)? * 60.,
            "move_screen_ratio" => self.screen_ratio = numeric(0.05, 1.)?,
            "zoom_inc_factor" => self.zoom_factor = numeric(1.01, 2.)?,
            "scroll_response" => self.scroll_response = numeric(0.06, 0.4)?,
            "wheel_scroll_multiplier" => self.wheel_multiplier = numeric(0.25, 4.)?,
            "smooth_scroll" => self.smooth_scroll = boolean()?,
            "render_scale" => self.render_scale = numeric(1., 2.5)?,
            "render_sharpness" => self.render_sharpness = numeric(0., 1.)?,
            "ui_animations" => self.animations = boolean()?,
            "rounded_corners" => self.rounded_corners = boolean()?,
            "document_theme" => self.document_theme = boolean()?,
            "open_in_new_windows" => {
                boolean()?;
            }
            "dark_mode_contrast" => self.dark_contrast = numeric(0.1, 1.)?,
            "font_size" => self.font_size = numeric(6., 48.)?,
            "page_padding" => self.page_padding = numeric(0., 64.)?,
            "max_page_width" => self.max_page_width = numeric(400., 4000.)?,
            "flat_toc" => self.flat_toc = boolean()?,
            "sort_bookmarks_by_location" => self.sort_bookmarks = boolean()?,
            "restore_last_position" => self.restore_position = boolean()?,
            "show_statusbar" => self.statusbar = boolean()?,
            "show_highlights" => self.highlights = boolean()?,
            "text_highlight_color" => self.selection_color = parse_color(value)?,
            "search_highlight_color" => self.search_color = parse_color(value)?,
            key if key.starts_with("highlight_color_") => {
                if let Some(kind) = key
                    .strip_prefix("highlight_color_")
                    .filter(|s| s.len() == 1)
                    .and_then(|s| s.bytes().next())
                    .filter(u8::is_ascii_lowercase)
                {
                    self.highlight_colors[(kind - b'a') as usize] = parse_color(value)?;
                }
            }
            _ => {} // Retain original/extension settings without discarding their values.
        }
        self.values.insert(key.into(), value.into());
        if key == "dark_mode" {
            self.values.insert(
                "default_dark_mode".into(),
                if self.dark { "1" } else { "0" }.into(),
            );
        }
        if key == "move_amount" {
            self.values.insert(
                "vertical_move_amount".into(),
                format!("{}", self.move_amount / 60.),
            );
        }
        Ok(())
    }
    pub fn set_preference(&mut self, key: &str, value: &str) -> Result<()> {
        self.set_preferences(&[(key.to_string(), value.to_string())])
    }
    pub fn set_preferences(&mut self, changes: &[(String, String)]) -> Result<()> {
        let mut next = self.clone();
        for (key, value) in changes {
            next.apply_preference(key, value)?;
        }
        let directory = self
            .directory
            .as_deref()
            .context("No configuration directory")?;
        std::fs::create_dir_all(directory)?;
        let path = directory.join("prefs_user.config");
        let original =
            optional_read(&path)?.unwrap_or_else(|| "# Omapdf user preferences\n".into());
        let replacements: BTreeMap<_, _> = changes
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let mut written = std::collections::BTreeSet::new();
        let mut output = String::new();
        for line in original.lines() {
            let existing = line.split_whitespace().next().unwrap_or("");
            // Normalize legacy aliases when writing their canonical Omapdf name.
            if (existing == "dark_mode" && replacements.contains_key("default_dark_mode"))
                || (existing == "move_amount" && replacements.contains_key("vertical_move_amount"))
            {
                continue;
            }
            if let Some(value) = replacements.get(existing) {
                if written.insert(existing) {
                    output.push_str(&format!("{existing} {value}\n"));
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }
        for (key, value) in &replacements {
            if !written.contains(key) {
                output.push_str(&format!("{key} {value}\n"));
            }
        }
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(output.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path)?;
        *self = next;
        Ok(())
    }
    pub fn value(&self, key: &str) -> String {
        self.values
            .get(key)
            .map(|value| value.split(" #").next().unwrap_or(value).trim().to_string())
            .unwrap_or_default()
    }
    pub fn parse_keys(&mut self, text: &str) -> Result<()> {
        for (line_number, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((command, keys)) = line.split_once(char::is_whitespace) else {
                bail!("Line {}: expected COMMAND KEYS", line_number + 1);
            };
            let keys = keys.trim();
            if keys == "<unbound>" {
                self.bindings.retain(|_, value| value != command);
                continue;
            }
            self.bindings.insert(parse_sequence(keys)?, command.into());
        }
        Ok(())
    }

    pub fn binding(&self, sequence: &[String]) -> Binding<'_> {
        if let Some(command) = self.bindings.get(sequence) {
            Binding::Exact(command)
        } else if self.bindings.keys().any(|key| key.starts_with(sequence)) {
            Binding::Prefix
        } else {
            Binding::Missing
        }
    }
}

/// Normalize Omapdf's nested modifier notation (`<C-<pagedown>>`).
pub fn parse_sequence(text: &str) -> Result<Vec<String>> {
    let mut keys = vec![];
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '<' {
            keys.push(character.to_string());
            continue;
        }
        let mut token = String::new();
        let mut depth = 1;
        for character in characters.by_ref() {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => token.push(character),
            }
        }
        if depth != 0 {
            bail!("Unclosed key token in {text}");
        }
        let mut parts: Vec<_> = token.split('-').map(str::to_string).collect();
        let mut key = parts.pop().context("Empty key token")?;
        let mut modifiers = vec![];
        for modifier in &parts {
            match modifier.as_str() {
                "C" => modifiers.push("ctrl"),
                "A" => modifiers.push("alt"),
                "S" => modifiers.push("shift"),
                _ => bail!("Unknown modifier {modifier}"),
            }
        }
        if key.chars().count() == 1 && modifiers.contains(&"shift") {
            key = key.to_uppercase();
            modifiers.retain(|&m| m != "shift");
        }
        if key == "space" {
            key = " ".into();
        }
        modifiers.sort_unstable();
        keys.push(format!(
            "{}{key}",
            if modifiers.is_empty() {
                String::new()
            } else {
                format!("{}-", modifiers.join("-"))
            }
        ));
    }
    if keys.is_empty() {
        bail!("Empty key sequence");
    }
    Ok(keys)
}

fn optional_read(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn parse_color(value: &str) -> Result<u32> {
    let values = value
        .split_whitespace()
        .take(3)
        .map(str::parse::<f32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    anyhow::ensure!(
        values.len() == 3
            && values
                .iter()
                .all(|n| n.is_finite() && (0.0..=1.0).contains(n)),
        "Expected three RGB values between 0 and 1"
    );
    Ok((((values[0] * 255.).round() as u32) << 24)
        | (((values[1] * 255.).round() as u32) << 16)
        | (((values[2] * 255.).round() as u32) << 8)
        | 0x66)
}
