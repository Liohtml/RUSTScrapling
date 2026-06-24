use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CheckpointData {
    pub pending_urls: Vec<String>,
    pub seen_fingerprints: Vec<String>,
    pub items_count: u64,
}

impl CheckpointManager {
    pub fn new(dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            checkpoint_dir: path,
        })
    }

    pub fn save(&self, data: &CheckpointData) -> Result<(), std::io::Error> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let json = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
        // Write atomically via a temp file in the same directory: `persist`
        // does an atomic replace on POSIX (rename) and Windows (MoveFileExW
        // with REPLACE_EXISTING), and the temp file is auto-removed on drop if
        // anything fails before persist — so a crash mid-write cannot corrupt
        // or zero-out an existing checkpoint, and no orphan .tmp is left behind.
        let mut tmp = NamedTempFile::new_in(&self.checkpoint_dir)?;
        tmp.write_all(json.as_bytes())?;
        tmp.persist(&file_path).map_err(|e| e.error)?;
        Ok(())
    }

    pub fn restore(&self) -> Option<CheckpointData> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let data = std::fs::read_to_string(&file_path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn cleanup(&self) {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let _ = std::fs::remove_file(file_path);
    }

    pub fn exists(&self) -> bool {
        self.checkpoint_dir.join("checkpoint.json").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample(items: u64) -> CheckpointData {
        CheckpointData {
            pending_urls: vec!["https://example.com".into()],
            seen_fingerprints: vec![],
            items_count: items,
        }
    }

    #[test]
    fn save_then_restore_roundtrips() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        assert!(!mgr.exists());
        mgr.save(&sample(42)).unwrap();
        assert!(mgr.exists());
        let restored = mgr.restore().unwrap();
        assert_eq!(restored.items_count, 42);
        assert_eq!(
            restored.pending_urls,
            vec!["https://example.com".to_string()]
        );
    }

    #[test]
    fn save_overwrites_and_leaves_no_tmp_files() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        mgr.save(&sample(1)).unwrap();
        // Overwrite — atomic replace must succeed on every platform.
        mgr.save(&sample(2)).unwrap();
        assert_eq!(mgr.restore().unwrap().items_count, 2);

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["checkpoint.json".to_string()]);
    }

    #[test]
    fn cleanup_removes_checkpoint() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        mgr.save(&sample(7)).unwrap();
        assert!(mgr.exists());
        mgr.cleanup();
        assert!(!mgr.exists());
    }
}
