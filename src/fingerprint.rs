//! Cache content hashes by file revision without changing persistent document IDs.
use crate::reload::Revision;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
};

#[derive(Serialize, Deserialize)]
struct Fingerprint {
    revision: Revision,
    content: String,
    reflow: String,
}

pub(crate) fn document_ids(
    file: &mut File,
    path: &Path,
    revision: Revision,
    typography: &[u8],
    cache: Option<&Path>,
) -> Result<(String, String)> {
    let cache_file = cache.map(|directory| {
        let mut key = Sha256::new();
        key.update(path.as_os_str().as_bytes());
        key.update([0]);
        key.update(typography);
        directory.join(format!("{:x}.json", key.finalize()))
    });
    if let Some(cached) = cache_file.as_ref().and_then(|path| {
        let mut bytes = Vec::new();
        File::open(path)
            .ok()?
            .take(4096)
            .read_to_end(&mut bytes)
            .ok()?;
        serde_json::from_slice::<Fingerprint>(&bytes).ok()
    }) && cached.revision == revision
        && [&cached.content, &cached.reflow].iter().all(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
    {
        return Ok((cached.content, cached.reflow));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    ensure!(
        Revision::from_metadata(&file.metadata()?) == revision,
        "Document changed while hashing"
    );
    let content = format!("{:x}", hash.clone().finalize());
    hash.update(typography);
    let reflow = format!("{:x}", hash.finalize());
    if let Some(path) = cache_file {
        // A missing/unwritable cache must never prevent opening a document.
        let _ = (|| -> Result<()> {
            let parent = path.parent().unwrap();
            std::fs::create_dir_all(parent)?;
            let mut staging = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer(
                &mut staging,
                &Fingerprint {
                    revision,
                    content: content.clone(),
                    reflow: reflow.clone(),
                },
            )?;
            staging.flush()?;
            staging.persist(path)?;
            Ok(())
        })();
    }
    Ok((content, reflow))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom};

    #[test]
    fn preserves_ids_skips_unchanged_bytes_and_invalidates_same_size_edits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("book.pdf");
        let cache = directory.path().join("cache");
        std::fs::write(&path, b"original").unwrap();
        let mut file = File::open(&path).unwrap();
        let revision = Revision::from_metadata(&file.metadata().unwrap());
        let ids = document_ids(&mut file, &path, revision, b"options", Some(&cache)).unwrap();
        assert_eq!(ids.0, format!("{:x}", Sha256::digest(b"original")));
        assert_eq!(ids.1, format!("{:x}", Sha256::digest(b"originaloptions")));
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            document_ids(&mut file, &path, revision, b"options", Some(&cache)).unwrap(),
            ids
        );
        assert_eq!(
            file.stream_position().unwrap(),
            0,
            "cache hit must not reread the book"
        );
        let modified = file.metadata().unwrap().modified().unwrap();
        std::fs::write(&path, b"modified").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let mut file = File::open(&path).unwrap();
        let revision = Revision::from_metadata(&file.metadata().unwrap());
        let changed = document_ids(&mut file, &path, revision, b"options", Some(&cache)).unwrap();
        assert_ne!(
            changed, ids,
            "ctime detects edits even when size and mtime match"
        );
        let entry = std::fs::read_dir(&cache)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(entry, b"invalid cache").unwrap();
        file.rewind().unwrap();
        assert_eq!(
            document_ids(&mut file, &path, revision, b"options", Some(&cache)).unwrap(),
            changed
        );
        file.rewind().unwrap();
        assert_eq!(
            document_ids(&mut file, &path, revision, b"options", Some(&path)).unwrap(),
            changed,
            "unwritable cache is optional"
        );
    }
}
