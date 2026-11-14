//! Lossless reader backups and legacy JSON/SQLite annotation migration.
use crate::{
    document::{Document, EpubOptions},
    reload::Revision,
    state::{AnnotationMeta, Bookmark, Highlight, Library, Location, Portal},
};
use anyhow::{Context, Result, bail, ensure};
use md5::{Digest, Md5};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Default)]
pub struct Report {
    pub library: Library,
    pub issues: Vec<String>,
}
#[derive(Default, Debug, PartialEq)]
pub struct Counts {
    pub documents: usize,
    pub bookmarks: usize,
    pub highlights: usize,
    pub portals: usize,
    pub marks: usize,
    pub updated: usize,
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}
fn number(value: &Value, key: &str, default: f32) -> Result<f32> {
    let Some(value) = value.get(key).filter(|value| !value.is_null()) else {
        return Ok(default);
    };
    let number = value
        .as_f64()
        .with_context(|| format!("{key} is not a number"))?;
    ensure!(
        number.is_finite() && number.abs() <= 1e12,
        "Invalid {key} coordinate"
    );
    Ok(number as f32)
}
fn character(value: &Value, key: &str) -> Result<char> {
    let value = value.get(key).with_context(|| format!("Missing {key}"))?;
    if let Some(text) = value.as_str() {
        let mut chars = text.chars();
        let first = chars.next().with_context(|| format!("Empty {key}"))?;
        ensure!(chars.next().is_none(), "{key} must be a single character");
        return Ok(first);
    }
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .and_then(char::from_u32)
        .with_context(|| format!("Invalid {key} character"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        _ => bail!("{key} must be an array"),
    }
}

pub fn checksum(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let revision = Revision::from_metadata(&file.metadata()?);
    let mut hash = Md5::new();
    let mut bytes = [0u8; 65536];
    loop {
        let read = file.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        hash.update(&bytes[..read]);
    }
    ensure!(
        Revision::read(path)? == revision,
        "Document changed while computing its original checksum"
    );
    Ok(format!("{:x}", hash.finalize()))
}
fn page_origin(doc: &Document, page: usize) -> f64 {
    doc.pages
        .iter()
        .take(page)
        .map(|page| page.height as f64)
        .sum()
}
fn page_at(doc: &Document, y: f32) -> (usize, f32) {
    let mut origin = 0.;
    for (index, page) in doc.pages.iter().enumerate() {
        let end = origin + page.height as f64;
        if y as f64 <= end || index + 1 == doc.pages.len() {
            return (index, (y as f64 - origin) as f32);
        }
        origin = end;
    }
    (0, 0.)
}
fn location(doc: &Document, x: f32, y: f32, zoom: f32) -> Location {
    let (page, y) = page_at(doc, y.max(0.));
    let width = doc.pages[page].width;
    Location {
        document: doc.id.clone(),
        page,
        zoom: if zoom > 0. { zoom.clamp(0.2, 5.) } else { 1. },
        fit: zoom <= 0.,
        offset: [-x / width, -y / width],
    }
}
fn absolute(doc: &Document, location: &Location) -> Result<(f32, f32)> {
    let page = doc
        .pages
        .get(location.page)
        .context("Saved location is outside the document")?;
    Ok((
        -location.offset[0] * page.width,
        (page_origin(doc, location.page) - location.offset[1] as f64 * page.width as f64) as f32,
    ))
}

struct Resolver<'a> {
    known: &'a Library,
    base: PathBuf,
    options: EpubOptions,
    hashes: BTreeMap<PathBuf, String>,
    documents: BTreeMap<PathBuf, Arc<Document>>,
}
impl Resolver<'_> {
    fn load(&mut self, record: &Value) -> Result<Arc<Document>> {
        let expected = string(record, "checksum").to_ascii_lowercase();
        ensure!(
            expected.is_empty()
                || (expected.len() == 32 && expected.bytes().all(|byte| byte.is_ascii_hexdigit())),
            "Invalid original document checksum"
        );
        let mut candidates = Vec::new();
        for path in std::iter::once(string(record, "path")).chain(
            record
                .get("paths")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        ) {
            if path.is_empty() {
                continue;
            }
            let path = PathBuf::from(path);
            let path = if path.is_absolute() {
                path
            } else {
                self.base.join(path)
            };
            if !candidates.contains(&path) {
                candidates.push(path);
            }
        }
        if !expected.is_empty() {
            for state in self.known.documents.values() {
                if !candidates.contains(&state.path) {
                    candidates.push(state.path.clone());
                }
            }
        }
        let mut failures = Vec::new();
        for path in candidates {
            let attempt = (|| -> Result<Arc<Document>> {
                let path = path.canonicalize()?;
                if !expected.is_empty() {
                    let hash = match self.hashes.get(&path) {
                        Some(hash) => hash.clone(),
                        None => {
                            let hash = checksum(&path)?;
                            self.hashes.insert(path.clone(), hash.clone());
                            hash
                        }
                    };
                    ensure!(hash == expected, "Checksum mismatch");
                }
                if let Some(doc) = self.documents.get(&path) {
                    return Ok(doc.clone());
                }
                let doc = Arc::new(Document::open_preview(&path, self.options)?);
                if !expected.is_empty() {
                    ensure!(
                        checksum(&path)? == expected,
                        "Checksum changed while loading document"
                    );
                }
                self.documents.insert(path, doc.clone());
                Ok(doc)
            })();
            match attempt {
                Ok(doc) => return Ok(doc),
                Err(error) => failures.push(format!("{}: {error:#}", path.display())),
            }
        }
        bail!(
            "Cannot resolve {}: {}",
            string(record, "path"),
            failures.join("; ")
        )
    }
}

fn nearest_word(doc: &Document, page: usize, x: f32, y: f32) -> Result<usize> {
    doc.ensure_page_text(page)?;
    if let Some(index) = doc.pages[page].word_at(x, y) {
        return Ok(index);
    }
    doc.pages[page]
        .words
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let distance = |word: &crate::document::Word| {
                let r = word.rect;
                let dx = (r.x - x).max(x - r.x - r.width).max(0.);
                let dy = (r.y - y).max(y - r.y - r.height).max(0.);
                dx * dx + dy * dy
            };
            distance(a).total_cmp(&distance(b))
        })
        .map(|(index, _)| index)
        .context("Highlight page has no extractable text")
}
fn legacy_meta(doc: &Document, kind: &str, record: &Value) -> Result<AnnotationMeta> {
    let mut meta = AnnotationMeta::imported(&doc.id, record);
    if string(record, "uuid").is_empty() {
        // Older schemas have no UUID. Use the original annotation identity
        // rules, not mutable comments or timestamps, so re-import stays stable.
        let identity = match kind {
            "highlights" => json!([
                number(record, "selection_begin_x", 0.)?,
                number(record, "selection_begin_y", 0.)?,
                number(record, "selection_end_x", 0.)?,
                number(record, "selection_end_y", 0.)?
            ]),
            "bookmarks" => json!([
                number(record, "y_offset", number(record, "begin_y", 0.)?)?,
                string(record, "description")
            ]),
            "marks" => json!(character(record, "symbol")?),
            "portals" => json!([
                number(record, "src_offset_y", 0.)?,
                string(record, "dst_checksum")
            ]),
            _ => unreachable!(),
        };
        meta.uuid = AnnotationMeta::stable(&format!("{}:{kind}", doc.id), &identity).uuid;
    }
    Ok(meta)
}

fn legacy_highlights(doc: &Document, record: &Value) -> Result<Vec<Highlight>> {
    let begin_y = number(record, "selection_begin_y", 0.)?;
    let end_y = number(record, "selection_end_y", begin_y)?;
    let (begin_page, begin_y) = page_at(doc, begin_y);
    let (end_page, end_y) = page_at(doc, end_y);
    let begin = nearest_word(
        doc,
        begin_page,
        number(record, "selection_begin_x", 0.)? + doc.pages[begin_page].width / 2.,
        begin_y,
    )?;
    let end = nearest_word(
        doc,
        end_page,
        number(record, "selection_end_x", 0.)? + doc.pages[end_page].width / 2.,
        end_y,
    )?;
    let ((first_page, first), (last_page, last)) = if (begin_page, begin) <= (end_page, end) {
        ((begin_page, begin), (end_page, end))
    } else {
        ((end_page, end), (begin_page, begin))
    };
    let kind = character(record, "type")?;
    ensure!(
        kind.is_ascii_alphabetic() || kind == '_',
        "Unsupported highlight type"
    );
    let mut highlights = Vec::new();
    for page in first_page..=last_page {
        doc.ensure_page_text(page)?;
        let words = &doc.pages[page].words;
        if words.is_empty() {
            continue;
        }
        let start = if page == first_page { first } else { 0 };
        let end = if page == last_page {
            last + 1
        } else {
            words.len()
        };
        let selected = words
            .get(start..end)
            .context("Invalid highlight endpoints")?;
        let mut meta = legacy_meta(doc, "highlights", record)?;
        if first_page != last_page {
            meta.aliases.clear();
            meta.group = Some(meta.uuid.clone());
            meta.uuid = AnnotationMeta::stable(&meta.uuid, &json!(page)).uuid;
        }
        highlights.push(Highlight {
            meta,
            page,
            kind,
            text: if first_page == last_page && !string(record, "description").is_empty() {
                string(record, "description").into()
            } else {
                selected
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            },
            note: (!string(record, "text_annot").is_empty())
                .then(|| string(record, "text_annot").to_owned()),
            rects: selected.iter().map(|word| word.rect).collect(),
        });
    }
    Ok(highlights)
}

pub fn import_legacy(
    value: &Value,
    base: &Path,
    known: &Library,
    options: EpubOptions,
) -> Result<Report> {
    let records = value
        .get("documents")
        .and_then(Value::as_array)
        .context("Missing original documents array")?;
    let mut report = Report::default();
    let mut resolver = Resolver {
        known,
        base: base.into(),
        options,
        hashes: BTreeMap::new(),
        documents: BTreeMap::new(),
    };
    let mut documents = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        ensure!(
            record.is_object(),
            "Document record {index} is not an object"
        );
        match resolver.load(record) {
            Ok(doc) => {
                for key in [
                    string(record, "checksum"),
                    string(record, "legacy_key"),
                    string(record, "path"),
                ] {
                    if !key.is_empty() {
                        documents.insert(key.to_owned(), doc.clone());
                    }
                }
                documents.insert(format!("record:{index}"), doc.clone());
                report.library.touch(&doc.id, &doc.path);
            }
            Err(error) => report
                .issues
                .push(format!("Document {}: {error:#}", index + 1)),
        }
    }
    for (index, record) in records.iter().enumerate() {
        let Some(doc) = documents.get(&format!("record:{index}")) else {
            continue;
        };
        let result = (|| -> Result<()> {
            let at = location(
                doc,
                number(record, "offset_x", 0.)?,
                number(record, "offset_y", 0.)?,
                number(record, "zoom_level", 0.)?,
            );
            let state = report.library.documents.get_mut(&doc.id).unwrap();
            state.page = at.page;
            state.offset = at.offset;
            state.zoom = at.zoom;
            state.fit = at.fit;
            Ok(())
        })();
        if let Err(error) = result {
            report.issues.push(format!(
                "Reading position {}: {error:#}",
                doc.path.display()
            ));
        }
        for (key, label) in [
            ("bookmarks", "Bookmark"),
            ("marks", "Mark"),
            ("highlights", "Highlight"),
            ("portals", "Portal"),
        ] {
            let annotations = match array(record, key) {
                Ok(values) => values,
                Err(error) => {
                    report
                        .issues
                        .push(format!("{label} list {}: {error:#}", doc.path.display()));
                    continue;
                }
            };
            for (index, annotation) in annotations.iter().enumerate() {
                let result = (|| -> Result<()> {
                    ensure!(annotation.is_object(), "Annotation must be an object");
                    let meta = legacy_meta(doc, key, annotation)?;
                    match key {
                        "bookmarks" => {
                            let y =
                                number(annotation, "y_offset", number(annotation, "begin_y", 0.)?)?;
                            report.library.bookmarks.push(Bookmark {
                                meta,
                                name: string(annotation, "description").into(),
                                location: location(doc, 0., y, 0.),
                            });
                        }
                        "marks" => {
                            let symbol = character(annotation, "symbol")?;
                            ensure!(symbol.is_ascii_alphabetic(), "Mark must be a–z or A–Z");
                            let at = location(
                                doc,
                                number(annotation, "x_offset", 0.)?,
                                number(annotation, "y_offset", 0.)?,
                                number(annotation, "zoom_level", 0.)?,
                            );
                            if symbol.is_ascii_uppercase() {
                                report.library.global_marks.insert(symbol, at);
                                report.library.global_mark_metadata.insert(symbol, meta);
                            } else {
                                let state = report.library.documents.get_mut(&doc.id).unwrap();
                                state.marks.insert(symbol, at);
                                state.mark_metadata.insert(symbol, meta);
                            }
                        }
                        "highlights" => report
                            .library
                            .documents
                            .get_mut(&doc.id)
                            .unwrap()
                            .highlights
                            .extend(legacy_highlights(doc, annotation)?),
                        "portals" => {
                            let destination = if annotation.get("same").and_then(Value::as_bool)
                                == Some(true)
                            {
                                doc
                            } else {
                                documents.get(string(annotation,"dst_checksum")).context("Portal destination is unavailable; its original record is retained")?
                            };
                            let source = location(
                                doc,
                                number(annotation, "src_offset_x", 0.)?,
                                number(annotation, "src_offset_y", 0.)?,
                                0.,
                            );
                            let destination = location(
                                destination,
                                number(annotation, "dst_offset_x", 0.)?,
                                number(annotation, "dst_offset_y", 0.)?,
                                number(annotation, "dst_zoom_level", 0.)?,
                            );
                            report
                                .library
                                .documents
                                .get_mut(&doc.id)
                                .unwrap()
                                .portals
                                .push(Portal {
                                    meta,
                                    source_page: source.page,
                                    source_offset: Some(source.offset),
                                    destination,
                                });
                        }
                        _ => unreachable!(),
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    report.issues.push(format!(
                        "{label} {} in {}: {error:#}",
                        index + 1,
                        doc.path.display()
                    ));
                }
            }
        }
    }
    let mut source = value.clone();
    source["omapdf_import_base"] =
        json!(base.canonicalize().unwrap_or_else(|_| base.to_path_buf()));
    report.library.source_imports.push(source);
    report.library.normalize_ids();
    validate(&report.library)?;
    Ok(report)
}

fn validate_location(location: &Location) -> Result<()> {
    ensure!(
        location.page < 100_000
            && location.zoom.is_finite()
            && (0.0..=100.).contains(&location.zoom)
            && location
                .offset
                .iter()
                .all(|value| value.is_finite() && value.abs() <= 1_000_000.),
        "Invalid saved location"
    );
    Ok(())
}
pub fn validate(library: &Library) -> Result<()> {
    for state in library.documents.values() {
        validate_location(&Location {
            page: state.page,
            zoom: state.zoom,
            offset: state.offset,
            ..Default::default()
        })?;
        for at in state.marks.values() {
            validate_location(at)?;
        }
        for highlight in &state.highlights {
            ensure!(
                highlight.page < 100_000
                    && (highlight.kind.is_ascii_alphabetic() || highlight.kind == '_'),
                "Invalid highlight page/type"
            );
            ensure!(
                !highlight.rects.is_empty()
                    && highlight
                        .rects
                        .iter()
                        .all(|rect| [rect.x, rect.y, rect.width, rect.height]
                            .iter()
                            .all(|value| value.is_finite() && value.abs() <= 1e9)
                            && rect.width > 0.
                            && rect.height > 0.),
                "Invalid highlight geometry"
            );
        }
        for portal in &state.portals {
            ensure!(portal.source_page < 100_000, "Invalid portal source page");
            if let Some(offset) = portal.source_offset {
                ensure!(
                    offset
                        .iter()
                        .all(|value| value.is_finite() && value.abs() <= 1_000_000.),
                    "Invalid portal source offset"
                );
            }
            validate_location(&portal.destination)?;
        }
    }
    for (id, meta) in &library.deleted_annotations {
        ensure!(
            !id.is_empty() && meta.uuid == *id,
            "Invalid annotation deletion identity"
        );
    }
    for bookmark in &library.bookmarks {
        validate_location(&bookmark.location)?;
    }
    for at in library.global_marks.values() {
        validate_location(at)?;
    }
    Ok(())
}

/// Resolve retained original records after missing/moved documents become available.
pub fn retry(known: &Library, options: EpubOptions) -> Result<Report> {
    let mut result = Report::default();
    for source in &known.source_imports {
        let base = source
            .get("omapdf_import_base")
            .and_then(Value::as_str)
            .map(Path::new)
            .unwrap_or(Path::new("."));
        match import_legacy(source, base, known, options) {
            Ok(report) => {
                merge(&mut result.library, report.library)?;
                result.issues.extend(report.issues);
            }
            Err(error) => result.issues.push(format!("Retained import: {error:#}")),
        }
    }
    Ok(result)
}

pub fn read_paths(paths: &[PathBuf], known: &Library, options: EpubOptions) -> Result<Report> {
    ensure!(!paths.is_empty(), "Choose an import file");
    if paths.len() == 1 {
        return read(&paths[0], known, options);
    }
    let value = crate::legacy_sqlite::read_paths(paths)?;
    import_legacy(
        &value,
        paths[0].parent().unwrap_or(Path::new(".")),
        known,
        options,
    )
}

pub fn read(path: &Path, known: &Library, options: EpubOptions) -> Result<Report> {
    let sqlite = if path.is_dir() {
        true
    } else {
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("Cannot open import {}", path.display()))?;
        let mut header = [0u8; 16];
        let length = file.read(&mut header)?;
        length == 16 && header == *b"SQLite format 3\0"
    };
    if sqlite {
        let value = crate::legacy_sqlite::read(path)?;
        let base = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(Path::new("."))
        };
        return import_legacy(&value, base, known, options);
    }
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let native = if let Some(native) = value.get("omapdf") {
        ensure!(
            native.get("version").and_then(Value::as_u64) == Some(1),
            "Unsupported Omapdf export version"
        );
        Some(native.get("library").context("Missing exported library")?)
    } else if value.get("documents").is_some_and(Value::is_object)
        && value.get("bookmarks").is_some_and(Value::is_array)
    {
        Some(&value)
    } else {
        None
    };
    if let Some(native) = native {
        let mut library: Library = serde_json::from_value(native.clone())?;
        library.normalize_ids();
        validate(&library)?;
        return Ok(Report {
            library,
            issues: Vec::new(),
        });
    }
    import_legacy(
        &value,
        path.parent().unwrap_or(Path::new(".")),
        known,
        options,
    )
}

/// Add annotations by stable identity, updating newer imported revisions without
/// deleting local entries absent from a snapshot. Importing positions is explicit.
pub fn merge(library: &mut Library, mut incoming: Library) -> Result<Counts> {
    incoming.normalize_ids();
    library.normalize_ids();
    validate(&incoming)?;
    for (id, meta) in &incoming.deleted_annotations {
        if library
            .deleted_annotations
            .get(id)
            .is_none_or(|old| meta.newer_than(old))
        {
            library.deleted_annotations.insert(id.clone(), meta.clone());
        }
    }
    incoming.deleted_annotations = library.deleted_annotations.clone();
    incoming.prune_deleted();
    library.prune_deleted();
    let mut counts = Counts::default();
    fn merge_items<T: Clone + serde::Serialize + PartialEq>(
        target: &mut Vec<T>,
        incoming: Vec<T>,
        identity: impl Fn(&T) -> &AnnotationMeta,
        identity_mut: impl Fn(&mut T) -> &mut AnnotationMeta,
    ) -> (usize, usize) {
        let mut added = 0;
        let mut updated = 0;
        for mut item in incoming {
            let meta = identity(&item).clone();
            if let Some(existing) = target
                .iter_mut()
                .find(|existing| identity(existing).same_identity(&meta))
            {
                let old = identity(existing).clone();
                if item != *existing
                    && (meta.newer_than(&old)
                        || (meta.revision == 0
                            && old.revision == 0
                            && meta.modification_time == old.modification_time))
                {
                    let incoming_meta = identity(&item).clone();
                    let combined = identity_mut(&mut item);
                    let pdf_name = combined.pdf_name.clone();
                    combined.uuid = old.uuid.clone();
                    combined.absorb_identity(&incoming_meta);
                    combined.absorb_identity(&old);
                    if pdf_name.is_some() {
                        combined.pdf_name = pdf_name;
                    }
                    if !old.creation_time.is_empty()
                        && (combined.creation_time.is_empty()
                            || old.creation_time < combined.creation_time)
                    {
                        combined.creation_time = old.creation_time.clone();
                    }
                    *existing = item;
                    updated += 1;
                } else {
                    identity_mut(existing).absorb_identity(&meta);
                    if identity(existing) != &old {
                        updated += 1;
                    }
                }
            } else if !target.contains(&item) {
                target.push(item);
                added += 1;
            }
        }
        (added, updated)
    }
    for (id, state) in incoming.documents {
        if let Some(existing) = library.documents.get_mut(&id) {
            existing.page = state.page;
            existing.offset = state.offset;
            existing.zoom = state.zoom;
            existing.fit = state.fit;
            if state.path.is_file() || !existing.path.is_file() {
                existing.path = state.path;
            }
            let (added, updated) = merge_items(
                &mut existing.highlights,
                state.highlights,
                |highlight| &highlight.meta,
                |highlight| &mut highlight.meta,
            );
            counts.highlights += added;
            counts.updated += updated;
            let (added, updated) = merge_items(
                &mut existing.portals,
                state.portals,
                |portal| &portal.meta,
                |portal| &mut portal.meta,
            );
            counts.portals += added;
            counts.updated += updated;
            for (symbol, at) in state.marks {
                if !existing.marks.contains_key(&symbol) {
                    counts.marks += 1;
                }
                let incoming_meta = state.mark_metadata.get(&symbol);
                let current_meta = existing.mark_metadata.get(&symbol);
                let newer = match (incoming_meta, current_meta) {
                    (Some(a), Some(b)) => {
                        a.newer_than(b)
                            || (a.revision == 0
                                && b.revision == 0
                                && a.modification_time == b.modification_time)
                    }
                    (Some(_), None) => true,
                    _ => false,
                };
                if !existing.marks.contains_key(&symbol) || newer {
                    if existing.marks.get(&symbol).is_some_and(|old| old != &at) {
                        counts.updated += 1;
                    }
                    existing.marks.insert(symbol, at);
                    if let Some(meta) = incoming_meta {
                        existing.mark_metadata.insert(symbol, meta.clone());
                    }
                }
            }
        } else {
            counts.documents += 1;
            counts.highlights += state.highlights.len();
            counts.portals += state.portals.len();
            counts.marks += state.marks.len();
            library.documents.insert(id, state);
        }
    }
    let (added, updated) = merge_items(
        &mut library.bookmarks,
        incoming.bookmarks,
        |bookmark| &bookmark.meta,
        |bookmark| &mut bookmark.meta,
    );
    counts.bookmarks = added;
    counts.updated += updated;
    for (symbol, at) in incoming.global_marks {
        if !library.global_marks.contains_key(&symbol) {
            counts.marks += 1;
        }
        let incoming_meta = incoming.global_mark_metadata.get(&symbol);
        let current_meta = library.global_mark_metadata.get(&symbol);
        let newer = match (incoming_meta, current_meta) {
            (Some(a), Some(b)) => {
                a.newer_than(b)
                    || (a.revision == 0
                        && b.revision == 0
                        && a.modification_time == b.modification_time)
            }
            (Some(_), None) => true,
            _ => false,
        };
        if !library.global_marks.contains_key(&symbol) || newer {
            if library
                .global_marks
                .get(&symbol)
                .is_some_and(|old| old != &at)
            {
                counts.updated += 1;
            }
            library.global_marks.insert(symbol, at);
            if let Some(meta) = incoming_meta {
                library.global_mark_metadata.insert(symbol, meta.clone());
            }
        }
    }
    for id in incoming.recent.into_iter().rev() {
        library.recent.retain(|existing| *existing != id);
        library.recent.insert(0, id);
    }
    for source in incoming.source_imports {
        if !library.source_imports.contains(&source) {
            library.source_imports.push(source);
        }
    }
    Ok(counts)
}

fn add_meta(mut value: Value, meta: &AnnotationMeta) -> Value {
    if let (Some(value), Value::Object(meta)) = (
        value.as_object_mut(),
        serde_json::to_value(meta).expect("Serializable metadata"),
    ) {
        value.extend(meta);
    }
    value
}

/// The `documents` array is consumable by legacy. The versioned Omapdf
/// section retains every native field and unavailable-document record losslessly.
pub fn export_value(library: &Library, options: EpubOptions) -> Result<(Value, Vec<String>)> {
    validate(library)?;
    let mut library = library.clone();
    library.normalize_ids();
    let mut docs = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut issues = Vec::new();
    for (id, state) in &library.documents {
        let result = (|| -> Result<(Arc<Document>, String)> {
            let doc = Arc::new(Document::open_preview(&state.path, options)?);
            ensure!(
                doc.id == *id,
                "Saved document revision/layout no longer matches the file"
            );
            Ok((doc, checksum(&state.path)?))
        })();
        match result {
            Ok((doc, hash)) => {
                docs.insert(id.clone(), doc);
                hashes.insert(id.clone(), hash);
            }
            Err(error) => issues.push(format!(
                "Legacy export {}: {error:#}; native data retained",
                state.path.display()
            )),
        }
    }
    if !library.deleted_annotations.is_empty() {
        issues.push("Legacy JSON imports do not apply deletions; reader deletion records are retained in the native backup".into());
    }
    let mut records = Vec::new();
    for (id, doc) in &docs {
        let result = (|| -> Result<Value> {
            let state = &library.documents[id];
            let at = Location {
                document: id.clone(),
                page: state.page,
                offset: state.offset,
                zoom: state.zoom,
                fit: state.fit,
            };
            let (x, y) = absolute(doc, &at)?;
            let mut bookmarks = Vec::new();
            let mut marks = Vec::new();
            let mut highlights = Vec::new();
            let mut portals = Vec::new();
            for bookmark in library
                .bookmarks
                .iter()
                .filter(|bookmark| bookmark.location.document == *id)
            {
                let (_, y) = absolute(doc, &bookmark.location)?;
                bookmarks.push(add_meta(json!({"y_offset":y,"description":bookmark.name,"begin_x":-1.,"begin_y":-1.,"end_x":-1.,"end_y":-1.}),&bookmark.meta));
            }
            for (symbol, at) in state.marks.iter().chain(
                library
                    .global_marks
                    .iter()
                    .filter(|(_, at)| at.document == *id),
            ) {
                let (x, y) = absolute(doc, at)?;
                let meta = if symbol.is_ascii_uppercase() {
                    library.global_mark_metadata.get(symbol)
                } else {
                    state.mark_metadata.get(symbol)
                };
                let value =
                    json!({"symbol":*symbol as u32,"y_offset":y,"x_offset":x,"zoom_level":at.zoom});
                marks.push(
                    meta.map(|meta| add_meta(value.clone(), meta))
                        .unwrap_or(value),
                );
            }
            for fragments in state.highlight_groups() {
                let first_highlight = fragments[0];
                let last_highlight = fragments[fragments.len() - 1];
                let first_page = doc
                    .pages
                    .get(first_highlight.page)
                    .context("Highlight page is outside its document")?;
                let last_page = doc
                    .pages
                    .get(last_highlight.page)
                    .context("Highlight page is outside its document")?;
                let first = first_highlight
                    .rects
                    .first()
                    .context("Highlight has no geometry")?;
                let last = last_highlight
                    .rects
                    .last()
                    .context("Highlight has no geometry")?;
                let mut meta = first_highlight.meta.clone();
                if let Some(group) = &meta.group {
                    meta.uuid = group.clone();
                    meta.aliases.clear();
                }
                meta.group = None;
                let text = fragments
                    .iter()
                    .map(|highlight| highlight.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                highlights.push(add_meta(json!({"type":first_highlight.kind as u32,"description":text,"text_annot":first_highlight.note.clone().unwrap_or_default(),
                "selection_begin_x":first.x-first_page.width/2.+0.5,"selection_begin_y":page_origin(doc, first_highlight.page) as f32+first.y+first.height/2.,
                "selection_end_x":last.x+last.width-last_page.width/2.-0.5,"selection_end_y":page_origin(doc, last_highlight.page) as f32+last.y+last.height/2.}),&meta));
            }
            for portal in &state.portals {
                if let (Some(destination), Some(hash)) = (
                    docs.get(&portal.destination.document),
                    hashes.get(&portal.destination.document),
                ) {
                    let (x, y) = absolute(destination, &portal.destination)?;
                    let source = Location {
                        page: portal.source_page,
                        offset: portal.source_offset.unwrap_or_default(),
                        ..Default::default()
                    };
                    let (src_x, src_y) = absolute(doc, &source)?;
                    portals.push(add_meta(json!({"src_offset_x":src_x,"src_offset_y":src_y,"dst_checksum":hash,"dst_offset_x":x,"dst_offset_y":y,
                    "dst_zoom_level":portal.destination.zoom,"same":portal.destination.document==*id}),&portal.meta));
                } else {
                    issues.push(format!("Portal destination {} unavailable for legacy export; native record retained",portal.destination.document));
                }
            }
            Ok(
                json!({"path":doc.path,"checksum":hashes[id],"zoom_level":state.zoom,"offset_x":x,"offset_y":y,
            "bookmarks":bookmarks,"marks":marks,"highlights":highlights,"portals":portals}),
            )
        })();
        match result {
            Ok(record) => records.push(record),
            Err(error) => issues.push(format!(
                "Legacy record {}: {error:#}; native data retained",
                doc.path.display()
            )),
        }
    }
    Ok((
        json!({"documents":records,"omapdf":{"version":1,"library":library}}),
        issues,
    ))
}

fn same_file(a: &Path, b: &Path) -> bool {
    let resolved = |path: &Path| {
        path.parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .ok()
            .and_then(|parent| path.file_name().map(|name| parent.join(name)))
    };
    if let (Some(a), Some(b)) = (resolved(a), resolved(b))
        && a == b
    {
        return true;
    }

    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize())
        && a == b
    {
        return true;
    }
    #[cfg(unix)]
    if let (Ok(a), Ok(b)) = (std::fs::metadata(a), std::fs::metadata(b)) {
        use std::os::unix::fs::MetadataExt;
        return a.dev() == b.dev() && a.ino() == b.ino();
    }
    false
}

/// Atomically write an export, backing up a previous JSON export before replacing
/// it. An existing non-JSON file (including a PDF/database) is never overwritten.
pub fn write(
    path: &Path,
    library: &Library,
    options: EpubOptions,
    active_library: &Path,
) -> Result<Vec<String>> {
    ensure!(
        !same_file(path, active_library),
        "Export cannot replace the active reader library"
    );
    let (value, issues) = export_value(library, options)?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if path.try_exists()? {
        let previous = std::fs::read(path)?;
        let old: Value =
            serde_json::from_slice(&previous).context("Refusing to overwrite a non-JSON file")?;
        ensure!(
            old.get("documents").is_some(),
            "Refusing to overwrite an unrelated JSON file"
        );
        let mut backup = tempfile::Builder::new()
            .prefix("omapdf-export-backup-")
            .suffix(".json")
            .tempfile_in(parent)?;
        use std::io::Write;
        backup.write_all(&previous)?;
        backup.as_file().sync_all()?;
        backup.keep()?;
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, &value)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(issues)
}
