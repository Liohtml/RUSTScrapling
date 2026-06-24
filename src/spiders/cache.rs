use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

pub struct ResponseCache {
    cache_dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub url: String,
    pub headers: HashMap<String, String>,
}

impl ResponseCache {
    pub fn new(cache_dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(cache_dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self { cache_dir: path })
    }

    pub async fn get(&self, url: &str) -> Option<CachedResponse> {
        let file_path = self.cache_path(url);
        // Async read so the Tokio worker thread is not blocked on disk I/O.
        let data = tokio::fs::read_to_string(&file_path).await.ok()?;
        serde_json::from_str(&data).ok()
    }

    pub async fn put(&self, url: &str, response: &CachedResponse) -> Result<(), std::io::Error> {
        let file_path = self.cache_path(url);
        let cache_dir = self.cache_dir.clone();
        let data = serde_json::to_string_pretty(response).map_err(std::io::Error::other)?;
        // The atomic temp-file + persist write is synchronous (NamedTempFile
        // has no async API), so run it on the blocking pool to avoid stalling
        // a Tokio worker thread. `persist` still does an atomic replace on both
        // POSIX (rename) and Windows (MoveFileExW with REPLACE_EXISTING), and
        // the temp file is auto-removed on drop if anything fails.
        tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            let mut tmp = NamedTempFile::new_in(&cache_dir)?;
            tmp.write_all(data.as_bytes())?;
            tmp.persist(&file_path).map_err(|e| e.error)?;
            Ok(())
        })
        .await
        .map_err(std::io::Error::other)?
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.cache_dir.join(format!("{}.json", hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample(url: &str) -> CachedResponse {
        CachedResponse {
            status: 200,
            content_type: "text/html".to_string(),
            body: "<html></html>".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn put_then_get_roundtrips() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        cache
            .put("https://example.com", &sample("https://example.com"))
            .await
            .unwrap();
        let got = cache.get("https://example.com").await.unwrap();
        assert_eq!(got.status, 200);
        assert_eq!(got.url, "https://example.com");
    }

    #[tokio::test]
    async fn put_overwrites_existing_entry_and_leaves_no_tmp_files() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        // Two writes to the same URL — the second must atomically replace the
        // first (this is the path that fails on Windows with plain rename).
        cache
            .put("https://example.com", &sample("https://example.com"))
            .await
            .unwrap();
        let mut updated = sample("https://example.com");
        updated.body = "<html>updated</html>".to_string();
        cache.put("https://example.com", &updated).await.unwrap();
        assert_eq!(
            cache.get("https://example.com").await.unwrap().body,
            "<html>updated</html>"
        );

        // Only the final .json should remain — no orphaned temp files.
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries.len(), 1, "unexpected files: {:?}", entries);
        assert!(entries[0].ends_with(".json"));
    }

    #[tokio::test]
    async fn get_returns_none_for_missing() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        assert!(cache.get("https://missing.example").await.is_none());
    }
}
