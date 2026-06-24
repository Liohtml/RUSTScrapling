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

    pub fn get(&self, url: &str) -> Option<CachedResponse> {
        let file_path = self.cache_path(url);
        let data = std::fs::read_to_string(&file_path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn put(&self, url: &str, response: &CachedResponse) -> Result<(), std::io::Error> {
        let file_path = self.cache_path(url);
        let data = serde_json::to_string_pretty(response).map_err(std::io::Error::other)?;
        // Write atomically: a temp file in the same directory keeps the rename
        // same-filesystem, and `persist` performs an atomic replace on both
        // POSIX (rename) and Windows (MoveFileExW with REPLACE_EXISTING). The
        // temp file is auto-removed on drop if anything fails before persist.
        let mut tmp = NamedTempFile::new_in(&self.cache_dir)?;
        tmp.write_all(data.as_bytes())?;
        tmp.persist(&file_path).map_err(|e| e.error)?;
        Ok(())
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

    #[test]
    fn put_then_get_roundtrips() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        cache
            .put("https://example.com", &sample("https://example.com"))
            .unwrap();
        let got = cache.get("https://example.com").unwrap();
        assert_eq!(got.status, 200);
        assert_eq!(got.url, "https://example.com");
    }

    #[test]
    fn put_overwrites_existing_entry_and_leaves_no_tmp_files() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        // Two writes to the same URL — the second must atomically replace the
        // first (this is the path that fails on Windows with plain rename).
        cache
            .put("https://example.com", &sample("https://example.com"))
            .unwrap();
        let mut updated = sample("https://example.com");
        updated.body = "<html>updated</html>".to_string();
        cache.put("https://example.com", &updated).unwrap();
        assert_eq!(
            cache.get("https://example.com").unwrap().body,
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

    #[test]
    fn get_returns_none_for_missing() {
        let dir = tempdir().unwrap();
        let cache = ResponseCache::new(dir.path().to_str().unwrap()).unwrap();
        assert!(cache.get("https://missing.example").is_none());
    }
}
