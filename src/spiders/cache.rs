use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

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
        let data = serde_json::to_string_pretty(response)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&file_path, data)
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.cache_dir.join(format!("{}.json", hash))
    }
}
