//! Stable revision detection for in-place writes and atomic file replacement.
use std::{fs::Metadata, path::Path, time::SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Revision {
    modified: Option<SystemTime>,
    length: u64,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
impl Revision {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        std::fs::metadata(path).map(|metadata| Self::from_metadata(&metadata))
    }
    pub fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            length: metadata.len(),
            #[cfg(unix)]
            identity: {
                use std::os::unix::fs::MetadataExt;
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            },
        }
    }
}

pub struct Watch {
    loaded: Revision,
    candidate: Option<Revision>,
}
impl Watch {
    pub fn new(loaded: Revision) -> Self {
        Self {
            loaded,
            candidate: None,
        }
    }
    /// Require two identical observations before attempting to read new content.
    /// Do not advance `loaded` on failed reads: a repaired file must be retried.
    pub fn observe(&mut self, revision: Option<Revision>) -> bool {
        if revision.is_none() || revision == Some(self.loaded) {
            self.candidate = None;
            return false;
        }
        let stable = self.candidate == revision;
        self.candidate = revision;
        stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_for_stable_writes_and_recovers_after_missing_or_invalid_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document");
        std::fs::write(&path, "original").unwrap();
        let loaded = Revision::read(&path).unwrap();
        let mut watch = Watch::new(loaded);
        assert!(!watch.observe(Some(loaded)));
        std::fs::write(&path, "partial").unwrap();
        let partial = Revision::read(&path).unwrap();
        assert!(!watch.observe(Some(partial)));
        std::fs::write(&path, "complete new content").unwrap();
        let complete = Revision::read(&path).unwrap();
        assert!(!watch.observe(Some(complete)));
        assert!(watch.observe(Some(complete)));
        assert!(
            watch.observe(Some(complete)),
            "failed loads remain retryable"
        );
        assert!(!watch.observe(None));
        assert!(!watch.observe(Some(complete)));
        assert!(watch.observe(Some(complete)));
        let mut watch = Watch::new(complete);
        assert!(!watch.observe(Some(complete)));
    }
    #[test]
    fn atomic_replacement_is_detected_even_with_identical_size_and_mtime() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document");
        std::fs::write(&path, "before").unwrap();
        let original = Revision::read(&path).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let replacement = directory.path().join("replacement");
        std::fs::write(&replacement, "after!").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&replacement)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert_ne!(Revision::read(&path).unwrap(), original);
    }
}
