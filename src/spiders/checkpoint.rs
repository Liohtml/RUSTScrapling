//! Crawl checkpoints for pause/resume: on pause the engine persists the
//! scheduler's pending requests and dedup set as `checkpoint.json`
//! (written atomically), and the next run restores them instead of
//! starting over. See
//! [`CrawlerEngine::request_pause`](crate::spiders::engine::CrawlerEngine::request_pause).

use super::request::SpiderRequest;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Reads and writes the `checkpoint.json` file in a crawl's checkpoint
/// directory.
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
}

/// The persisted state of a paused crawl.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CheckpointData {
    /// Full pending requests — method, headers, body, meta, priority and all
    /// — as of the pause. This is what current versions write.
    #[serde(default)]
    pub pending_requests: Vec<SpiderRequest>,
    /// Bare pending URLs. Kept only so a checkpoint written by an older
    /// version (pre-`pending_requests`) still restores instead of resuming
    /// with an empty queue; current versions leave this empty and use
    /// `pending_requests` instead.
    #[serde(default)]
    pub pending_urls: Vec<String>,
    /// The scheduler's dedup set: one fingerprint per unique URL enqueued
    /// before the pause, so a resumed crawl does not re-visit them.
    pub seen_fingerprints: Vec<String>,
    /// Number of items scraped before the pause (informational).
    pub items_count: u64,
}

impl CheckpointManager {
    /// Open a checkpoint directory, creating it if needed.
    pub fn new(dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            checkpoint_dir: path,
        })
    }

    /// Persist `data` as `checkpoint.json`, atomically replacing any
    /// previous checkpoint (a crash mid-write cannot corrupt the old one).
    pub async fn save(&self, data: &CheckpointData) -> Result<(), std::io::Error> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let dir = self.checkpoint_dir.clone();
        // Compact encoding: with the seen set persisted, checkpoints grow
        // with the crawl (one fingerprint per unique URL), and pretty-printed
        // indentation roughly doubles the file size for no benefit.
        let json = serde_json::to_string(data).map_err(std::io::Error::other)?;
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

    /// Load the stored checkpoint, or `None` when there is none or it
    /// cannot be read/parsed.
    pub async fn restore(&self) -> Option<CheckpointData> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let data = tokio::fs::read_to_string(&file_path).await.ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Delete the checkpoint file (called after a crawl completes so the
    /// next run starts fresh). Missing files are ignored.
    pub async fn cleanup(&self) {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let _ = tokio::fs::remove_file(file_path).await;
    }

    /// Whether a checkpoint file exists.
    #[must_use]
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
            pending_requests: vec![SpiderRequest::new("https://example.com")],
            pending_urls: vec![],
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
        assert_eq!(restored.pending_requests.len(), 1);
        assert_eq!(restored.pending_requests[0].url(), "https://example.com");
    }

    #[tokio::test]
    async fn restore_falls_back_to_pending_urls_for_pre_upgrade_checkpoints() {
        // Simulates a checkpoint written before `pending_requests` existed:
        // only the `pending_urls`/`seen_fingerprints`/`items_count` fields
        // are present in the JSON. `#[serde(default)]` must still let this
        // deserialize, with `pending_requests` coming back empty.
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_str().unwrap()).unwrap();
        let legacy_json = r#"{"pending_urls":["https://example.com/old"],"seen_fingerprints":[],"items_count":5}"#;
        std::fs::write(dir.path().join("checkpoint.json"), legacy_json).unwrap();

        let restored = mgr.restore().await.expect("legacy checkpoint must parse");
        assert_eq!(restored.items_count, 5);
        assert!(restored.pending_requests.is_empty());
        assert_eq!(restored.pending_urls, vec!["https://example.com/old"]);
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
