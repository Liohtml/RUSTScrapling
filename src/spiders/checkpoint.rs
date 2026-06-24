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

    pub async fn save(&self, data: &CheckpointData) -> Result<(), std::io::Error> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let dir = self.checkpoint_dir.clone();
        let json = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
        // The atomic temp-file + persist write is synchronous, so run it on the
        // blocking pool to avoid stalling the async runtime (this runs in the
        // main crawl loop). `persist` does an atomic replace on POSIX (rename)
        // and Windows (MoveFileExW with REPLACE_EXISTING), and the temp file is
        // auto-removed on drop if anything fails — so a crash mid-write cannot
        // corrupt or zero-out an existing checkpoint, and no orphan .tmp leaks.
        tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            let mut tmp = NamedTempFile::new_in(&dir)?;
            tmp.write_all(json.as_bytes())?;
            tmp.persist(&file_path).map_err(|e| e.error)?;
            Ok(())
        })
        .await
        .map_err(std::io::Error::other)?
    }

    pub async fn restore(&self) -> Option<CheckpointData> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let data = tokio::fs::read_to_string(&file_path).await.ok()?;
        serde_json::from_str(&data).ok()
    }

    pub async fn cleanup(&self) {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let _ = tokio::fs::remove_file(file_path).await;
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

    #[tokio::test]
    async fn save_then_restore_roundtrips() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        assert!(!mgr.exists());
        mgr.save(&sample(42)).await.unwrap();
        assert!(mgr.exists());
        let restored = mgr.restore().await.unwrap();
        assert_eq!(restored.items_count, 42);
        assert_eq!(
            restored.pending_urls,
            vec!["https://example.com".to_string()]
        );
    }

    #[tokio::test]
    async fn save_overwrites_and_leaves_no_tmp_files() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        mgr.save(&sample(1)).await.unwrap();
        // Overwrite — atomic replace must succeed on every platform.
        mgr.save(&sample(2)).await.unwrap();
        assert_eq!(mgr.restore().await.unwrap().items_count, 2);

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["checkpoint.json".to_string()]);
    }

    #[tokio::test]
    async fn cleanup_removes_checkpoint() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        mgr.save(&sample(7)).await.unwrap();
        assert!(mgr.exists());
        mgr.cleanup().await;
        assert!(!mgr.exists());
    }
}
