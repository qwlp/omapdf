use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct Word {
    pub text: String,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub rect: Rect,
    pub target: String,
}

#[derive(Clone, Debug)]
pub struct Outline {
    pub title: String,
    pub page: usize,
    pub depth: usize,
    pub position: Option<[f32; 2]>,
}

/// MuPDF exposes trailing UTF-16 NUL bookmark padding as replacement characters.
/// Strip only the suffix; meaningful Unicode and replacement characters inside
/// a title must retain their position.
pub fn clean_outline_title(title: &str) -> String {
    title
        .trim_end_matches(|c: char| c == '\0' || c == '\u{fffd}' || c.is_whitespace())
        .to_owned()
}

/// Immutable once loaded; cloned document snapshots share the same page index.
#[derive(Clone, Debug)]
pub struct SharedVec<T>(Arc<OnceLock<Vec<T>>>);
impl<T> Default for SharedVec<T> {
    fn default() -> Self {
        Self(Arc::new(OnceLock::new()))
    }
}
impl<T> SharedVec<T> {
    pub fn ready(&self) -> bool {
        self.0.get().is_some()
    }
    fn set(&self, values: Vec<T>) {
        let _ = self.0.set(values);
    }
}
impl<T> From<Vec<T>> for SharedVec<T> {
    fn from(values: Vec<T>) -> Self {
        Self(Arc::new(OnceLock::from(values)))
    }
}
impl<T> FromIterator<T> for SharedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        values.into_iter().collect::<Vec<_>>().into()
    }
}
impl<T> Deref for SharedVec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.0.get().map(Vec::as_slice).unwrap_or(&[])
    }
}
impl<'a, T> IntoIterator for &'a SharedVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Clone> SharedVec<T> {
    pub fn push(&mut self, item: T) {
        let slot = Arc::make_mut(&mut self.0);
        if slot.get().is_none() {
            let _ = slot.set(vec![]);
        }
        slot.get_mut().unwrap().push(item);
    }
}

#[derive(Clone, Debug)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub words: SharedVec<Word>,
    pub links: SharedVec<Link>,
}

impl Page {
    pub fn word_at(&self, x: f32, y: f32) -> Option<usize> {
        self.words.iter().position(|word| {
            x >= word.rect.x - 1.
                && x <= word.rect.x + word.rect.width + 1.
                && y >= word.rect.y - 1.
                && y <= word.rect.y + word.rect.height + 1.
        })
    }

    pub fn find_regex(&self, pattern: &regex::Regex) -> Vec<(usize, usize)> {
        let mut text = String::new();
        let mut ranges = vec![];
        for word in &self.words {
            if !text.is_empty() {
                text.push(' ');
            }
            let start = text.len();
            text.push_str(&word.text);
            ranges.push(start..text.len());
        }
        pattern
            .find_iter(&text)
            .filter(|matched| !matched.is_empty())
            .filter_map(|matched| {
                let start = ranges
                    .iter()
                    .position(|range| range.end > matched.start())?;
                let end = ranges
                    .iter()
                    .rposition(|range| range.start < matched.end())?
                    + 1;
                Some((start, end))
            })
            .collect()
    }
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn find(&self, query: &str) -> Vec<(usize, usize)> {
        let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        if terms.is_empty() {
            return vec![];
        }
        self.words
            .windows(terms.len())
            .enumerate()
            .filter_map(|(i, words)| {
                words
                    .iter()
                    .zip(&terms)
                    .all(|(word, term)| word.text.to_lowercase().contains(term))
                    .then_some((i, i + terms.len()))
            })
            .collect()
    }
}

/// A password stays in memory only while an unlocked document/session worker owns it.
struct Password(Vec<u8>);
impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
impl Drop for Password {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
impl Password {
    fn text(&self) -> &str {
        std::str::from_utf8(&self.0).expect("UTF-8 password")
    }
}
static PASSWORDS: std::sync::LazyLock<
    Mutex<std::collections::BTreeMap<PathBuf, std::sync::Weak<Password>>>,
> = std::sync::LazyLock::new(|| Mutex::new(std::collections::BTreeMap::new()));

#[derive(Debug)]
pub struct PasswordRequired {
    pub rejected: bool,
}
impl std::fmt::Display for PasswordRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.rejected {
            "Incorrect password; try again"
        } else {
            "This document requires a password"
        })
    }
}
impl std::error::Error for PasswordRequired {}

fn authenticate(native: &mut mupdf::Document, password: Option<&Password>) -> Result<()> {
    if native.needs_password()?
        && !native.authenticate(password.map(Password::text).unwrap_or(""))?
    {
        return Err(PasswordRequired {
            rejected: password.is_some(),
        }
        .into());
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Document {
    pub path: PathBuf,
    pub id: String,
    pub title: String,
    pub pages: Vec<Page>,
    pub outline: Vec<Outline>,
    pub revision: crate::reload::Revision,
    pub reflowable: bool,
    pub epub_options: EpubOptions,
    pub author: String,
    indexing: Arc<Mutex<()>>,
    password: Option<Arc<Password>>,
}

/// EPUB pagination is independent of screen zoom, so annotations have stable coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpubOptions {
    pub font_size: f32,
    pub line_height: f32,
    pub width: f32,
    pub height: f32,
}
impl Default for EpubOptions {
    fn default() -> Self {
        Self {
            font_size: 12.,
            line_height: 1.45,
            width: 420.,
            height: 595.,
        }
    }
}
impl EpubOptions {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            self.font_size.is_finite()
                && (8.0..=28.).contains(&self.font_size)
                && self.line_height.is_finite()
                && (1.0..=2.).contains(&self.line_height)
                && self.width.is_finite()
                && (240.0..=900.).contains(&self.width)
                && self.height.is_finite()
                && (320.0..=1200.).contains(&self.height),
            "Invalid EPUB typography settings"
        );
        Ok(self)
    }
}

// MuPDF clones share the style context, including the mutable user CSS pointer.
// Concurrent set_user_css calls can double-free it; chapter parsing can also read
// freed CSS or another book's typography. Hold this guard through native access,
// and reapply each document's options before accessing its lazy EPUB chapters.
// A background indexer releases this lock between pages. Fair handoff prevents
// it immediately reacquiring the lock and starving foreground opens/renders.
static NATIVE_ACCESS: parking_lot::FairMutex<()> = parking_lot::FairMutex::new(());
type NativeAccess<'a> = parking_lot::FairMutexGuard<'a, ()>;

pub(crate) fn native_access(options: EpubOptions) -> Result<NativeAccess<'static>> {
    let guard = NATIVE_ACCESS.lock();
    options.validate()?;
    let mut context = mupdf::Context::get();
    context.set_text_aa_level(8);
    context.set_graphics_aa_level(8);
    context.set_user_css(&format!("body {{ line-height: {}; margin: 24pt; }} img, svg {{ max-width: 100%; }} pre {{ white-space: pre-wrap; }}", options.line_height))?;
    Ok(guard)
}

fn open_native(
    path: &Path,
    options: EpubOptions,
    _access: &NativeAccess<'_>,
    password: Option<&Password>,
) -> Result<mupdf::Document> {
    let mut document = mupdf::Document::open(path)?;
    authenticate(&mut document, password)?;
    if document.is_reflowable()? {
        document.layout(options.width, options.height, options.font_size)?;
    }
    Ok(document)
}

impl Document {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_options(path, EpubOptions::default())
    }
    pub fn open_with_options(path: &Path, options: EpubOptions) -> Result<Self> {
        Self::open_impl(path, options, true)
    }
    pub fn open_preview(path: &Path, options: EpubOptions) -> Result<Self> {
        Self::open_impl(path, options, false)
    }
    pub fn unlock_preview(path: &Path, options: EpubOptions, password: String) -> Result<Self> {
        Self::open_authenticated(
            path,
            options,
            false,
            Some(Arc::new(Password(password.into_bytes()))),
        )
    }
    pub(crate) fn reopen_native(&self, access: &NativeAccess<'_>) -> Result<mupdf::Document> {
        open_native(
            &self.path,
            self.epub_options,
            access,
            self.password.as_deref(),
        )
    }
    pub(crate) fn authenticate_native(&self, native: &mut mupdf::Document) -> Result<()> {
        authenticate(native, self.password.as_deref())
    }
    fn open_impl(path: &Path, options: EpubOptions, eager: bool) -> Result<Self> {
        let canonical = path.canonicalize()?;
        let password = PASSWORDS
            .lock()
            .map_err(|_| anyhow::anyhow!("Document session unavailable"))?
            .get(&canonical)
            .and_then(std::sync::Weak::upgrade);
        Self::open_authenticated(&canonical, options, eager, password)
    }
    fn open_authenticated(
        path: &Path,
        options: EpubOptions,
        eager: bool,
        password: Option<Arc<Password>>,
    ) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("Cannot open {}", path.display()))?;
        let mut file = std::fs::File::open(&path)?;
        let revision = crate::reload::Revision::from_metadata(&file.metadata()?);
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let access = native_access(options)?;
        let native = open_native(&path, options, &access, password.as_deref())
            .with_context(|| format!("Cannot read {}", path.display()))?;
        let reflowable = native.is_reflowable()?;
        if reflowable {
            hash.update(serde_json::to_vec(&options)?);
        }
        let id = format!("{:x}", hash.finalize());
        let title = native
            .metadata(mupdf::MetadataName::Title)
            .ok()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let author = native
            .metadata(mupdf::MetadataName::Author)
            .unwrap_or_default();
        let total = native.page_count()?;
        anyhow::ensure!(
            total > 0 && total <= 100_000,
            "Document has no pages or exceeds the page limit"
        );
        let mut pages = Vec::with_capacity(total as usize);
        for index in 0..total {
            let page = native.load_page(index)?;
            let bounds = page.bounds()?;
            let width = bounds.x1 - bounds.x0;
            let height = bounds.y1 - bounds.y0;
            anyhow::ensure!(
                width.is_finite() && height.is_finite() && width > 0. && height > 0.,
                "Invalid page geometry"
            );
            let content = Page {
                width,
                height,
                words: SharedVec::default(),
                links: SharedVec::default(),
            };
            if eager || reflowable || index == 0 {
                Self::extract_page(&native, index as usize, &content)?;
            }
            pages.push(content);
        }

        fn flatten(
            source: &[mupdf::Outline],
            depth: usize,
            output: &mut Vec<Outline>,
            total: usize,
        ) {
            for entry in source {
                // Group headings may have no destination of their own. Keep their
                // level and use the first navigable child rather than dropping them.
                fn first_destination(
                    entry: &mupdf::Outline,
                    total: usize,
                ) -> Option<mupdf::link::LinkDestination> {
                    entry
                        .dest
                        .filter(|dest| (dest.loc.page_number as usize) < total)
                        .or_else(|| {
                            entry
                                .down
                                .iter()
                                .find_map(|child| first_destination(child, total))
                        })
                }
                if let Some(dest) = first_destination(entry, total) {
                    let page = dest.loc.page_number as usize;
                    if page < total {
                        output.push(Outline {
                            title: clean_outline_title(&entry.title),
                            page,
                            depth,
                            position: match dest.kind {
                                mupdf::DestinationKind::XYZ { left, top, .. } => {
                                    Some([left.unwrap_or(0.), top.unwrap_or(0.)])
                                }
                                _ => None,
                            },
                        });
                    }
                }
                flatten(&entry.down, depth + 1, output, total);
            }
        }
        let mut outline = vec![];
        flatten(&native.outlines()?, 0, &mut outline, pages.len());
        anyhow::ensure!(
            crate::reload::Revision::read(&path)? == revision,
            "Document changed while opening; waiting for a complete write"
        );
        if let Some(password) = &password {
            let mut sessions = PASSWORDS
                .lock()
                .map_err(|_| anyhow::anyhow!("Document session unavailable"))?;
            sessions.retain(|_, value| value.strong_count() > 0);
            sessions.insert(path.clone(), Arc::downgrade(password));
        }
        Ok(Self {
            password,
            path,
            id,
            title,
            pages,
            outline,
            revision,
            reflowable,
            epub_options: options,
            author,
            indexing: Arc::new(Mutex::new(())),
        })
    }
    pub fn text_ready(&self, page: usize) -> bool {
        self.pages
            .get(page)
            .is_some_and(|page| page.words.ready() && page.links.ready())
    }
    pub fn indexed_pages(&self) -> usize {
        self.pages.iter().filter(|page| page.words.ready()).count()
    }
    pub fn ensure_page_text(&self, page: usize) -> Result<()> {
        if self.text_ready(page) {
            return Ok(());
        }
        let access = native_access(self.epub_options)?;
        let native = open_native(
            &self.path,
            self.epub_options,
            &access,
            self.password.as_deref(),
        )?;
        Self::extract_page(
            &native,
            page,
            self.pages.get(page).context("Page out of range")?,
        )
    }
    pub fn ensure_all_text(&self) -> Result<()> {
        self.ensure_all_text_until(&std::sync::atomic::AtomicBool::new(false))
    }
    pub fn ensure_all_text_until(&self, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
        if self.indexed_pages() == self.pages.len() {
            return Ok(());
        }
        let _indexing = self
            .indexing
            .lock()
            .map_err(|_| anyhow::anyhow!("Text index worker failed"))?;
        if self.indexed_pages() == self.pages.len() {
            return Ok(());
        }
        let native = {
            let access = native_access(self.epub_options)?;
            open_native(
                &self.path,
                self.epub_options,
                &access,
                self.password.as_deref(),
            )?
        };
        for (index, page) in self.pages.iter().enumerate() {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if !self.text_ready(index) {
                let _access = native_access(self.epub_options)?;
                Self::extract_page(&native, index, page)?;
            }
        }
        Ok(())
    }
    fn extract_page(native: &mupdf::Document, index: usize, output: &Page) -> Result<()> {
        let page = native.load_page(index as i32)?;
        let bounds = page.bounds()?;
        let rect = |r: mupdf::Rect| Rect {
            x: r.x0 - bounds.x0,
            y: r.y0 - bounds.y0,
            width: r.x1 - r.x0,
            height: r.y1 - r.y0,
        };
        let words = page
            .words(mupdf::TextExtractOptions::default())?
            .into_iter()
            .map(|word| Word {
                text: word.text,
                rect: rect(word.bounds),
            })
            .collect();
        let links = page
            .links()?
            .map(|link| {
                let target = link
                    .dest
                    .map(|dest| {
                        if let mupdf::DestinationKind::XYZ { left, top, .. } = dest.kind {
                            format!(
                                "#{}@{},{}",
                                dest.loc.page_number + 1,
                                left.unwrap_or(0.),
                                top.unwrap_or(0.)
                            )
                        } else {
                            format!("#{}", dest.loc.page_number + 1)
                        }
                    })
                    .unwrap_or(link.uri);
                Link {
                    rect: rect(link.bounds),
                    target,
                }
            })
            .collect();
        output.links.set(links);
        output.words.set(words);
        Ok(())
    }

    pub fn render(&self, page: usize, width: u32, directory: &Path, dark: bool) -> Result<PathBuf> {
        anyhow::ensure!(page < self.pages.len(), "Page out of range");
        std::fs::create_dir_all(directory)?;
        let path = directory.join(format!("{}-{page}-{width}-{dark}.png", self.id));
        if !path.exists() {
            let renderer = crate::raster::Rasterizer::new();
            let rendered = renderer.render(std::sync::Arc::new(self.clone()), page, width, dark)?;
            let mut bytes = rendered.as_bytes(0).context("Empty raster")?.to_vec();
            for pixel in bytes.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let size = rendered.size(0);
            image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, bytes)
                .context("Invalid raster")?
                .save(&path)?;
            prune_cache(directory, &path);
        }
        Ok(path)
    }

    /// Relocate a saved book position by nearby text when EPUB pagination changes.
    pub fn remap_location(
        &self,
        previous: &Self,
        location: &crate::state::Location,
    ) -> crate::state::Location {
        let mut mapped = location.clone();
        mapped.document = self.id.clone();
        let fraction = location.page as f32 / previous.pages.len().max(1) as f32;
        mapped.page = ((fraction * self.pages.len() as f32) as usize).min(self.pages.len() - 1);
        if let Some(old_page) = previous.pages.get(location.page) {
            let y = -location.offset[1] * old_page.width;
            let word = old_page
                .words
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (a.rect.y - y).abs().total_cmp(&(b.rect.y - y).abs()))
                .map(|(i, _)| i)
                .unwrap_or(0);
            for length in [6, 3, 1] {
                let text = old_page.words[word..(word + length).min(old_page.words.len())]
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some((page, start, _)) = self.find_text_near(&text, fraction) {
                    mapped.page = page;
                    mapped.offset = [
                        0.,
                        -self.pages[page].words[start].rect.y / self.pages[page].width,
                    ];
                    break;
                }
            }
        }
        mapped
    }
    pub fn find_text_near(&self, text: &str, fraction: f32) -> Option<(usize, usize, usize)> {
        self.pages
            .iter()
            .enumerate()
            .flat_map(|(page, content)| {
                content
                    .find(text)
                    .into_iter()
                    .map(move |(start, end)| (page, start, end))
            })
            .min_by(|a, b| {
                (a.0 as f32 / self.pages.len() as f32 - fraction)
                    .abs()
                    .total_cmp(&(b.0 as f32 / self.pages.len() as f32 - fraction).abs())
            })
    }

    /// Resolve common figure, table, section, equation and bibliography references.
    pub fn smart_target(&self, page: usize, index: usize) -> Option<(usize, usize)> {
        let words = &self.pages.get(page)?.words;
        let word = &words.get(index)?.text;
        let reference = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
        let label = reference.to_lowercase();
        let labels = [
            "figure", "fig.", "fig", "table", "section", "sec.", "equation", "eq.",
        ];
        let (kind, number) = if labels.contains(&label.as_str()) {
            (
                Some(label),
                words
                    .get(index + 1)?
                    .text
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
                    .trim_end_matches('.')
                    .to_string(),
            )
        } else if index > 0 && labels.contains(&words[index - 1].text.to_lowercase().as_str()) {
            (
                Some(words[index - 1].text.to_lowercase()),
                reference.trim_end_matches('.').to_string(),
            )
        } else if word.starts_with('[') || word.starts_with('(') {
            (
                None,
                word.trim_matches(|c: char| !c.is_ascii_digit()).to_string(),
            )
        } else {
            return None;
        };
        if number.is_empty() {
            return None;
        }
        let mut candidates = vec![];
        for (p, content) in self.pages.iter().enumerate() {
            for (i, word) in content.words.iter().enumerate() {
                if p == page && i.abs_diff(index) < 3 {
                    continue;
                }
                let matches = if let Some(kind) = &kind {
                    word.text
                        .to_lowercase()
                        .starts_with(kind.trim_end_matches('.'))
                        && content.words.get(i + 1).is_some_and(|w| {
                            w.text
                                .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
                                .trim_end_matches('.')
                                == number
                        })
                } else {
                    word.text == format!("[{number}]") || word.text == format!("({number})")
                };
                if matches {
                    let line_start =
                        i == 0 || (content.words[i - 1].rect.y - word.rect.y).abs() > 3.;
                    candidates.push((!line_start, p.abs_diff(page), p, i));
                }
            }
        }
        candidates.sort_unstable();
        candidates.first().map(|&(_, _, p, i)| (p, i))
    }
}

fn prune_cache(directory: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? != "png" {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some((metadata.modified().ok()?, metadata.len(), path))
        })
        .collect();
    let mut bytes: u64 = files.iter().map(|(_, length, _)| length).sum();
    files.sort_unstable();
    for (modified, length, path) in files {
        if bytes <= 256 * 1024 * 1024 {
            break;
        }
        if path != current
            && modified.elapsed().is_ok_and(|age| age.as_secs() > 5)
            && std::fs::remove_file(path).is_ok()
        {
            bytes = bytes.saturating_sub(length);
        }
    }
}

pub fn parse_metadata(xml: &str, pages: &mut [Page]) -> Result<Vec<Outline>> {
    let tree = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?;
    let mut outline = vec![];
    for node in tree.descendants().filter(|n| n.has_tag_name("item")) {
        if let Some(page) = node
            .attribute("page")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|p| p.checked_sub(1))
            .filter(|p| *p < pages.len())
        {
            outline.push(Outline {
                title: node
                    .children()
                    .filter_map(|child| child.text())
                    .collect::<String>()
                    .trim()
                    .to_string(),
                page,
                position: None,
                depth: node
                    .ancestors()
                    .filter(|n| n.has_tag_name("item"))
                    .count()
                    .saturating_sub(1),
            });
        }
    }
    for node in tree.descendants().filter(|n| n.has_tag_name("page")) {
        let Some(page) = node
            .attribute("number")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|p| p.checked_sub(1))
            .and_then(|p| pages.get_mut(p))
        else {
            continue;
        };
        for text in node.children().filter(|n| n.has_tag_name("text")) {
            for anchor in text.descendants().filter(|n| n.has_tag_name("a")) {
                if let Some(target) = anchor.attribute("href") {
                    let number = |key| {
                        text.attribute(key)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0.)
                    };
                    page.links.push(Link {
                        target: target.to_string(),
                        rect: Rect {
                            x: number("left"),
                            y: number("top"),
                            width: number("width"),
                            height: number("height"),
                        },
                    });
                }
            }
        }
    }
    Ok(outline)
}

pub fn parse_bbox(xml: &str) -> Result<Vec<Page>> {
    let tree = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?;
    let number = |node: roxmltree::Node<'_, '_>, name| -> Result<f32> {
        node.attribute(name)
            .with_context(|| format!("Missing {name}"))?
            .parse()
            .context("Invalid PDF coordinate")
    };
    tree.descendants()
        .filter(|n| n.has_tag_name("page"))
        .map(|node| {
            let width = number(node, "width")?;
            let height = number(node, "height")?;
            anyhow::ensure!(width > 0. && height > 0., "Invalid page size");
            let words = node
                .descendants()
                .filter(|n| n.has_tag_name("word"))
                .map(|word| {
                    let x = number(word, "xMin")?;
                    let y = number(word, "yMin")?;
                    Ok(Word {
                        text: word.text().unwrap_or_default().into(),
                        rect: Rect {
                            x,
                            y,
                            width: number(word, "xMax")? - x,
                            height: number(word, "yMax")? - y,
                        },
                    })
                })
                .collect::<Result<_>>()?;
            Ok(Page {
                width,
                height,
                words,
                links: vec![].into(),
            })
        })
        .collect()
}
