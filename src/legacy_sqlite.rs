//! Read-only snapshots of Omapdf's combined or split SQLite databases.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn snapshot(path: &Path) -> Result<BTreeMap<String, Vec<Value>>> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Cannot read Omapdf database {}", path.display()))?;
    connection.busy_timeout(std::time::Duration::from_secs(2))?;
    connection.pragma_update(None, "trusted_schema", false)?;
    let transaction = connection.unchecked_transaction()?;
    let names = {
        let mut statement = transaction.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut tables = BTreeMap::new();
    for name in names {
        let query = format!("SELECT * FROM \"{}\"", name.replace('"', "\"\""));
        let mut statement = transaction.prepare(&query)?;
        let columns = statement
            .column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let mut rows = statement.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let mut record = Map::new();
            for (index, column) in columns.iter().enumerate() {
                let value = match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(value) => json!(value),
                    ValueRef::Real(value) => serde_json::Number::from_f64(value)
                        .map(Value::Number)
                        .unwrap_or_else(|| json!({"non_finite_float":value.to_string()})),
                    ValueRef::Text(bytes) => Value::String(
                        std::str::from_utf8(bytes)
                            .context("Database contains non-UTF-8 text")?
                            .into(),
                    ),
                    ValueRef::Blob(bytes) => {
                        json!({"blob_hex":bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>()})
                    }
                };
                record.insert(column.clone(), value);
            }
            records.push(Value::Object(record));
        }
        tables.insert(name, records);
    }
    transaction.commit()?;
    Ok(tables)
}

fn source_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_dir() {
        if path.join("shared.db").is_file() {
            let mut paths = vec![path.join("shared.db")];
            if path.join("local.db").is_file() {
                paths.push(path.join("local.db"));
            }
            return Ok(paths);
        }
        ensure!(
            path.join("test.db").is_file(),
            "Directory has no Omapdf shared.db or test.db"
        );
        return Ok(vec![path.join("test.db")]);
    }
    ensure!(
        path.is_file(),
        "Omapdf database does not exist: {}",
        path.display()
    );
    let parent = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let mut paths = vec![path.to_path_buf()];
    if name == "local.db" && parent.join("shared.db").is_file() {
        paths.insert(0, parent.join("shared.db"));
    }
    if name == "shared.db" && parent.join("local.db").is_file() {
        paths.push(parent.join("local.db"));
    }
    Ok(paths)
}

fn text<'a>(record: &'a Value, key: &str) -> &'a str {
    record.get(key).and_then(Value::as_str).unwrap_or("")
}
fn records<'a>(tables: &'a BTreeMap<String, Vec<Value>>, name: &str) -> &'a [Value] {
    tables.get(name).map(Vec::as_slice).unwrap_or_default()
}
fn document<'a>(documents: &'a mut BTreeMap<String, Value>, key: &str) -> &'a mut Value {
    documents.entry(key.into()).or_insert_with(|| json!({
        "legacy_key":key,"checksum":if key.len()==32 && key.bytes().all(|byte| byte.is_ascii_hexdigit()) {key} else {""},
        "path":"","bookmarks":[],"highlights":[],"marks":[],"portals":[],
        "offset_x":0.,"offset_y":0.,"zoom_level":1.,
    }))
}

/// Convert relational rows into the same input used for original JSON exports.
/// Keep complete raw snapshots too, including UUIDs, timestamps and extra tables.
pub fn read(path: &Path) -> Result<Value> {
    read_paths(&source_files(path)?)
}

/// Explicit paths support custom shared-database locations as well as defaults.
pub fn read_paths(paths: &[PathBuf]) -> Result<Value> {
    ensure!(!paths.is_empty(), "Choose at least one Omapdf database");
    let mut canonical = Vec::new();
    for path in paths {
        let path = path.canonicalize()?;
        if !canonical.contains(&path) {
            canonical.push(path);
        }
    }
    let mut sources = Vec::new();
    for path in canonical {
        sources.push((path.clone(), snapshot(&path)?));
    }
    ensure!(
        sources.iter().any(|(_, tables)| [
            "opened_books",
            "bookmarks",
            "marks",
            "highlights",
            "links"
        ]
        .iter()
        .any(|name| tables.contains_key(*name))),
        "This is not a recognized Omapdf annotation database"
    );
    let mut documents = BTreeMap::new();
    let mut paths_by_checksum: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (_, tables) in &sources {
        for record in records(tables, "document_hash") {
            let (checksum, path) = (text(record, "hash"), text(record, "path"));
            if !checksum.is_empty() && !path.is_empty() {
                let paths = paths_by_checksum.entry(checksum.into()).or_default();
                if !paths.iter().any(|existing| existing == path) {
                    paths.push(path.into());
                }
            }
        }
        for record in records(tables, "opened_books") {
            let key = text(record, "path");
            if key.is_empty() {
                continue;
            }
            let target = document(&mut documents, key);
            for key in ["offset_x", "offset_y", "zoom_level", "last_access_time"] {
                if let Some(value) = record.get(key).filter(|value| !value.is_null()) {
                    target[key] = value.clone();
                }
            }
        }
        for (table, array) in [
            ("bookmarks", "bookmarks"),
            ("marks", "marks"),
            ("highlights", "highlights"),
            ("links", "portals"),
        ] {
            for record in records(tables, table) {
                let key = text(
                    record,
                    if table == "links" {
                        "src_document"
                    } else {
                        "document_path"
                    },
                );
                if key.is_empty() {
                    continue;
                }
                let mut annotation = record.clone();
                let aliases: &[(&str, &str)] = match table {
                    "bookmarks" => &[("desc", "description"), ("offset_y", "y_offset")],
                    "marks" => &[("offset_y", "y_offset"), ("offset_x", "x_offset")],
                    "highlights" => &[
                        ("desc", "description"),
                        ("begin_x", "selection_begin_x"),
                        ("begin_y", "selection_begin_y"),
                        ("end_x", "selection_end_x"),
                        ("end_y", "selection_end_y"),
                    ],
                    _ => &[("dst_document", "dst_checksum")],
                };
                for (from, to) in aliases {
                    if let Some(value) = record.get(*from) {
                        annotation[*to] = value.clone();
                    }
                }
                if table == "links" {
                    let destination = text(record, "dst_document");
                    if !destination.is_empty() {
                        document(&mut documents, destination);
                    }
                }
                document(&mut documents, key)[array]
                    .as_array_mut()
                    .unwrap()
                    .push(annotation);
            }
        }
    }
    for (key, record) in &mut documents {
        if let Some(paths) = paths_by_checksum.get(key) {
            record["paths"] = json!(paths);
            record["path"] = json!(
                paths
                    .iter()
                    .find(|path| Path::new(path).is_file())
                    .or(paths.first())
                    .cloned()
                    .unwrap_or_default()
            );
        } else if Path::new(key).is_absolute() || key.contains(std::path::MAIN_SEPARATOR) {
            record["path"] = json!(key);
        }
    }
    Ok(
        json!({"documents":documents.into_values().collect::<Vec<_>>(),
        "sqlite_sources":sources.into_iter().map(|(path,tables)| json!({"path":path,"tables":tables})).collect::<Vec<_>>() }),
    )
}
