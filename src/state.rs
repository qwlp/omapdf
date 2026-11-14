use crate::document::Rect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Stable annotation identity and legacy synchronization timestamps.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AnnotationMeta {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub uuid: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub creation_time: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub modification_time: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_name: Option<String>,
    /// Logical annotation shared by page-local PDF fragments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub aliases: std::collections::BTreeSet<String>,
}
fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl AnnotationMeta {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let seed = serde_json::json!([
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_string(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ]);
        let mut meta = Self::stable("new", &seed);
        meta.touch();
        meta
    }
    pub fn stable(namespace: &str, value: &serde_json::Value) -> Self {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(namespace.as_bytes());
        hash.update(serde_json::to_vec(value).expect("JSON annotation identity"));
        let mut bytes: [u8; 32] = hash.finalize().into();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let hex = bytes[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Self {
            uuid: format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            ),
            ..Self::default()
        }
    }
    pub fn imported(namespace: &str, value: &serde_json::Value) -> Self {
        let field = |key: &str| {
            value
                .get(key)
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_owned()
        };
        let mut meta = Self {
            uuid: field("uuid"),
            group: value
                .get("group")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            creation_time: field("creation_time"),
            modification_time: field("modification_time"),
            pdf_name: value
                .get("pdf_name")
                .and_then(|value| value.as_str())
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            aliases: value
                .get("aliases")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect(),
            revision: value
                .get("revision")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
        };
        if meta.uuid.is_empty() {
            meta.uuid = Self::stable(namespace, value).uuid;
        }
        meta
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        !self.uuid.is_empty()
            && !other.uuid.is_empty()
            && (self.uuid == other.uuid
                || self.aliases.contains(&other.uuid)
                || other.aliases.contains(&self.uuid)
                || self.aliases.iter().any(|id| other.aliases.contains(id)))
    }
    pub fn absorb_identity(&mut self, other: &Self) {
        for id in std::iter::once(&other.uuid).chain(&other.aliases) {
            if !id.is_empty() && *id != self.uuid {
                self.aliases.insert(id.clone());
            }
        }
        if self.group.is_none() {
            self.group = other.group.clone();
        }
        if other.pdf_name.is_some() {
            self.pdf_name = other.pdf_name.clone();
        }
    }

    pub fn newer_than(&self, other: &Self) -> bool {
        if self.revision > 0 && other.revision > 0 {
            self.revision > other.revision
        } else {
            (self.modification_time.as_str(), self.revision)
                > (other.modification_time.as_str(), other.revision)
        }
    }

    pub fn touch(&mut self) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if self.creation_time.is_empty() {
            self.creation_time = now.clone();
        }
        self.modification_time = now;
        let micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros()
            .min(u64::MAX as u128) as u64;
        self.revision = micros.max(self.revision.saturating_add(1));
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Location {
    pub document: String,
    pub page: usize,
    pub zoom: f32,
    pub offset: [f32; 2],
    pub fit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bookmark {
    #[serde(default, flatten)]
    pub meta: AnnotationMeta,
    pub name: String,
    pub location: Location,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Highlight {
    #[serde(default, flatten)]
    pub meta: AnnotationMeta,
    pub page: usize,
    pub text: String,
    pub kind: char,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub rects: Vec<Rect>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Portal {
    #[serde(default, flatten)]
    pub meta: AnnotationMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_offset: Option<[f32; 2]>,
    pub source_page: usize,
    pub destination: Location,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DocumentState {
    pub path: PathBuf,
    pub page: usize,
    pub zoom: f32,
    pub offset: [f32; 2],
    pub fit: bool,
    pub marks: BTreeMap<char, Location>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mark_metadata: BTreeMap<char, AnnotationMeta>,
    pub highlights: Vec<Highlight>,
    pub portals: Vec<Portal>,
}

impl DocumentState {
    /// Ordered logical annotations, each containing page-local geometry in reading order.
    pub fn highlight_groups(&self) -> Vec<Vec<&Highlight>> {
        let mut indices = BTreeMap::new();
        let mut groups: Vec<Vec<&Highlight>> = Vec::new();
        for highlight in &self.highlights {
            let key = highlight
                .meta
                .group
                .as_deref()
                .unwrap_or(&highlight.meta.uuid);
            // Legacy annotations without identities are independent until normalized.
            if key.is_empty() {
                groups.push(vec![highlight]);
            } else {
                let index = *indices.entry(key).or_insert_with(|| {
                    groups.push(Vec::new());
                    groups.len() - 1
                });
                groups[index].push(highlight);
            }
        }
        for fragments in &mut groups {
            fragments.sort_by_key(|fragment| fragment.page);
        }
        groups
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Library {
    pub documents: BTreeMap<String, DocumentState>,
    pub recent: Vec<String>,
    pub bookmarks: Vec<Bookmark>,
    pub global_marks: BTreeMap<char, Location>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub global_mark_metadata: BTreeMap<char, AnnotationMeta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_imports: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub deleted_annotations: BTreeMap<String, AnnotationMeta>,
}

fn normalize_annotations<T: Serialize>(
    items: &mut [T],
    namespace: &str,
    meta: impl Fn(&mut T) -> &mut AnnotationMeta,
) {
    let mut seen = items
        .iter_mut()
        .map(|item| meta(item).uuid.clone())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::BTreeSet<_>>();
    for item in items {
        if !meta(item).uuid.is_empty() {
            continue;
        }
        let identity = AnnotationMeta::stable(
            namespace,
            &serde_json::to_value(&*item).expect("Serializable annotation identity"),
        );
        let mut id = identity.uuid.clone();
        let mut occurrence = 0;
        while !seen.insert(id.clone()) {
            occurrence += 1;
            id = AnnotationMeta::stable(&identity.uuid, &serde_json::json!(occurrence)).uuid;
        }
        meta(item).uuid = id;
    }
}

impl Library {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let mut library: Self = serde_json::from_slice(&bytes)
                    .with_context(|| format!("Invalid library: {}", path.display()))?;
                library.normalize_ids();
                library.prune_deleted();
                Ok(library)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }
    pub fn annotation_metadata(&self) -> impl Iterator<Item = &AnnotationMeta> {
        self.bookmarks
            .iter()
            .map(|item| &item.meta)
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.highlights.iter().map(|item| &item.meta)),
            )
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.portals.iter().map(|item| &item.meta)),
            )
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.mark_metadata.values()),
            )
            .chain(self.global_mark_metadata.values())
    }
    pub fn expand_deletions(&mut self) {
        let mut expanded = self.deleted_annotations.clone();
        for meta in self.deleted_annotations.values() {
            let mut ids = meta.aliases.clone();
            ids.insert(meta.uuid.clone());
            for id in &ids {
                if expanded.get(id).is_none_or(|old| meta.newer_than(old)) {
                    let mut copy = meta.clone();
                    copy.uuid = id.clone();
                    copy.aliases = ids.clone();
                    copy.aliases.remove(id);
                    expanded.insert(id.clone(), copy);
                }
            }
        }
        self.deleted_annotations = expanded;
    }

    pub fn is_deleted(&self, meta: &AnnotationMeta) -> bool {
        std::iter::once(&meta.uuid)
            .chain(&meta.aliases)
            .chain(meta.group.iter())
            .any(|id| {
                self.deleted_annotations
                    .get(id)
                    .is_some_and(|deleted| !meta.newer_than(deleted))
            })
    }

    pub fn prune_deleted(&mut self) {
        self.expand_deletions();
        let deleted = &self.deleted_annotations;
        let keep = |meta: &AnnotationMeta| {
            std::iter::once(&meta.uuid)
                .chain(&meta.aliases)
                .chain(meta.group.iter())
                .all(|id| {
                    deleted
                        .get(id)
                        .is_none_or(|deleted| meta.newer_than(deleted))
                })
        };
        self.bookmarks.retain(|item| keep(&item.meta));
        for state in self.documents.values_mut() {
            state.highlights.retain(|item| keep(&item.meta));
            state.portals.retain(|item| keep(&item.meta));
            let removed = state
                .mark_metadata
                .iter()
                .filter(|(_, meta)| !keep(meta))
                .map(|(symbol, _)| *symbol)
                .collect::<Vec<_>>();
            for symbol in removed {
                state.marks.remove(&symbol);
                state.mark_metadata.remove(&symbol);
            }
        }
        let removed = self
            .global_mark_metadata
            .iter()
            .filter(|(_, meta)| !keep(meta))
            .map(|(symbol, _)| *symbol)
            .collect::<Vec<_>>();
        for symbol in removed {
            self.global_marks.remove(&symbol);
            self.global_mark_metadata.remove(&symbol);
        }
    }

    /// Stable tombstones prevent stale readers/import snapshots from resurrecting deletions.
    pub fn delete_annotation(&mut self, uuid: &str) -> bool {
        self.normalize_ids();
        let meta = self
            .bookmarks
            .iter()
            .map(|item| &item.meta)
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.highlights.iter().map(|item| &item.meta)),
            )
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.portals.iter().map(|item| &item.meta)),
            )
            .chain(
                self.documents
                    .values()
                    .flat_map(|state| state.mark_metadata.values()),
            )
            .chain(self.global_mark_metadata.values())
            .find(|meta| meta.uuid == uuid)
            .cloned();
        if let Some(mut meta) = meta {
            meta.touch();
            if let Some(group) = &meta.group {
                let mut parent = meta.clone();
                parent.uuid = group.clone();
                parent.aliases.clear();
                self.deleted_annotations.insert(group.clone(), parent);
            }
            self.deleted_annotations.insert(uuid.to_owned(), meta);
            self.prune_deleted();
            true
        } else {
            false
        }
    }

    /// Edit a logical highlight once even when PDF geometry spans multiple pages.
    pub fn set_highlight_note(
        &mut self,
        document: &str,
        meta: &AnnotationMeta,
        note: &str,
    ) -> bool {
        let Some(state) = self.documents.get_mut(document) else {
            return false;
        };
        let mut edited = meta.clone();
        edited.touch();
        let mut found = false;
        for highlight in &mut state.highlights {
            if highlight.meta.same_identity(meta)
                || meta
                    .group
                    .as_ref()
                    .is_some_and(|group| highlight.meta.group.as_ref() == Some(group))
            {
                highlight.note = (!note.is_empty()).then(|| note.to_owned());
                highlight.meta.creation_time = edited.creation_time.clone();
                highlight.meta.modification_time = edited.modification_time.clone();
                highlight.meta.revision = edited.revision;
                found = true;
            }
        }
        found
    }

    pub fn normalize_ids(&mut self) {
        for (id, state) in &mut self.documents {
            normalize_annotations(&mut state.highlights, id, |highlight| &mut highlight.meta);
            normalize_annotations(&mut state.portals, id, |portal| &mut portal.meta);
            for (symbol, location) in &state.marks {
                state.mark_metadata.entry(*symbol).or_insert_with(|| {
                    AnnotationMeta::stable(id, &serde_json::json!([symbol, location]))
                });
            }
        }
        normalize_annotations(&mut self.bookmarks, "bookmark", |bookmark| {
            &mut bookmark.meta
        });
        for (symbol, location) in &self.global_marks {
            self.global_mark_metadata.entry(*symbol).or_insert_with(|| {
                AnnotationMeta::stable("global-mark", &serde_json::json!([symbol, location]))
            });
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, self)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }
    /// Keep a recoverable snapshot before explicit library import changes.
    pub fn backup(&self, path: &Path) -> Result<PathBuf> {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut backup = tempfile::Builder::new()
            .prefix("omapdf-library-backup-")
            .suffix(".json")
            .tempfile_in(parent)?;
        serde_json::to_writer_pretty(&mut backup, self)?;
        backup.as_file().sync_all()?;
        let (_, path) = backup.keep()?;
        Ok(path)
    }

    pub fn touch(&mut self, id: &str, path: &Path) {
        self.documents.entry(id.into()).or_default().path = path.into();
        self.recent.retain(|item| item != id);
        self.recent.insert(0, id.into());
    }
}

pub fn nearest_portal_index(
    doc: &crate::document::Document,
    current: &Location,
    portals: &[Portal],
) -> Option<usize> {
    if doc.pages.is_empty() {
        return None;
    }
    let mut origins = Vec::with_capacity(doc.pages.len());
    let mut y = 0.;
    for page in &doc.pages {
        origins.push(y);
        y += page.height as f64;
    }
    let position = |page: usize, offset: [f32; 2]| {
        let page = page.min(doc.pages.len() - 1);
        [
            -(offset[0] as f64) * doc.pages[page].width as f64,
            origins[page] - (offset[1] as f64) * doc.pages[page].width as f64,
        ]
    };
    let at = position(current.page, current.offset);
    portals
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let distance = |portal: &Portal| {
                let point = position(portal.source_page, portal.source_offset.unwrap_or_default());
                (point[0] - at[0]).powi(2) + (point[1] - at[1]).powi(2)
            };
            distance(a).total_cmp(&distance(b))
        })
        .map(|(index, _)| index)
}

#[derive(Default, Debug)]
pub struct History {
    back: Vec<Location>,
    forward: Vec<Location>,
}
impl History {
    pub fn push(&mut self, location: Location) {
        if self.back.last() != Some(&location) {
            self.back.push(location);
        }
        self.forward.clear();
    }
    pub fn back(&mut self, current: Location) -> Option<Location> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }
    pub fn forward(&mut self, current: Location) -> Option<Location> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
    }
}

/// Apply only this reader's edits to the latest library written by other tabs.
pub fn merge_library(baseline: &Library, edited: &Library, latest: Library) -> Library {
    let mut edited = edited.clone();
    let mut identities: BTreeMap<String, AnnotationMeta> = BTreeMap::new();
    for meta in latest.annotation_metadata() {
        for id in std::iter::once(&meta.uuid).chain(&meta.aliases) {
            identities
                .entry(id.clone())
                .and_modify(|old| old.absorb_identity(meta))
                .or_insert_with(|| meta.clone());
        }
    }
    for meta in edited.deleted_annotations.values_mut() {
        if let Some(current) = identities.get(&meta.uuid) {
            meta.absorb_identity(current);
        }
    }
    edited.expand_deletions();

    fn same_item(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        let ids = |value: &serde_json::Value| {
            value
                .get("uuid")
                .and_then(|value| value.as_str())
                .into_iter()
                .chain(
                    value
                        .get("aliases")
                        .and_then(|value| value.as_array())
                        .into_iter()
                        .flatten()
                        .filter_map(|value| value.as_str()),
                )
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let a_ids = ids(a);
        let b_ids = ids(b);
        if a_ids.is_empty() || b_ids.is_empty() {
            a == b
        } else {
            a_ids.iter().any(|id| b_ids.contains(id))
        }
    }
    fn merge(base: &serde_json::Value, edit: &serde_json::Value, latest: &mut serde_json::Value) {
        if base == edit {
            return;
        }
        if let (Some(edit), Some(latest)) = (edit.as_object(), latest.as_object_mut()) {
            let empty = serde_json::Map::new();
            let base = base.as_object().unwrap_or(&empty);
            for key in base.keys() {
                if !edit.contains_key(key) {
                    latest.remove(key);
                }
            }
            for (key, value) in edit {
                if base.get(key) == Some(value) {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "bookmarks"
                        | "highlights"
                        | "portals"
                        | "source_imports"
                        | "recent"
                        | "aliases"
                ) && let Some(edit) = value.as_array()
                {
                    let base = base
                        .get(key)
                        .and_then(|value| value.as_array())
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let latest = latest.entry(key).or_insert_with(|| serde_json::json!([]));
                    if let Some(latest) = latest.as_array_mut() {
                        if key == "recent" {
                            latest.retain(|value| !base.contains(value) || edit.contains(value));
                            for item in edit.iter().rev() {
                                latest.retain(|value| value != item);
                                latest.insert(0, item.clone());
                            }
                            continue;
                        }
                        latest.retain(|value| {
                            !base.iter().any(|old| same_item(value, old))
                                || edit.iter().any(|item| same_item(value, item))
                        });
                        for item in edit {
                            let old = base.iter().find(|old| same_item(old, item));
                            if old == Some(item) {
                                continue;
                            }
                            if let Some(index) =
                                latest.iter().position(|current| same_item(current, item))
                            {
                                merge(
                                    old.unwrap_or(&serde_json::Value::Null),
                                    item,
                                    &mut latest[index],
                                );
                            } else if old.is_none() {
                                latest.push(item.clone());
                            }
                            // A deletion in another reader wins over an edit of the same annotation.
                        }
                        continue;
                    }
                }
                merge(
                    base.get(key).unwrap_or(&serde_json::Value::Null),
                    value,
                    latest.entry(key).or_insert(serde_json::Value::Null),
                );
            }
        } else {
            *latest = edit.clone();
        }
    }
    let mut latest = serde_json::to_value(latest).expect("Serializable library");
    merge(
        &serde_json::to_value(baseline).expect("Serializable library"),
        &serde_json::to_value(&edited).expect("Serializable library"),
        &mut latest,
    );
    serde_json::from_value(latest).expect("Merged library")
}
