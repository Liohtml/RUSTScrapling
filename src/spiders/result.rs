use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

/// A list of scraped items (JSON values).
#[derive(Debug, Clone, Default)]
pub struct ItemList {
    items: Vec<serde_json::Value>,
}

impl ItemList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: serde_json::Value) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Write items as a JSON array to a file.
    pub fn to_json(&self, path: &Path, indent: usize) -> io::Result<()> {
        let buf = if indent > 0 {
            serde_json::to_vec_pretty(&self.items)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        } else {
            serde_json::to_vec(&self.items)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        };
        fs::write(path, buf)
    }

    /// Write items as JSON Lines to a file.
    pub fn to_jsonl(&self, path: &Path) -> io::Result<()> {
        let mut file = fs::File::create(path)?;
        for item in &self.items {
            let line = serde_json::to_string(item)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}

impl IntoIterator for ItemList {
    type Item = serde_json::Value;
    type IntoIter = std::vec::IntoIter<serde_json::Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Statistics gathered during a crawl.
#[derive(Debug, Clone)]
pub struct CrawlStats {
    pub requests_count: u64,
    pub concurrent_requests: u32,
    pub concurrent_requests_per_domain: u32,
    pub failed_requests_count: u64,
    pub offsite_requests_count: u64,
    pub robots_disallowed_count: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub response_bytes: u64,
    pub items_scraped: u64,
    pub items_dropped: u64,
    pub blocked_requests_count: u64,
    pub download_delay: f64,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub response_status_count: HashMap<u16, u64>,
    pub domains_response_bytes: HashMap<String, u64>,
    pub sessions_requests_count: HashMap<String, u64>,
    pub custom_stats: HashMap<String, serde_json::Value>,
}

impl Default for CrawlStats {
    fn default() -> Self {
        Self {
            requests_count: 0,
            concurrent_requests: 0,
            concurrent_requests_per_domain: 0,
            failed_requests_count: 0,
            offsite_requests_count: 0,
            robots_disallowed_count: 0,
            cache_hits: 0,
            cache_misses: 0,
            response_bytes: 0,
            items_scraped: 0,
            items_dropped: 0,
            blocked_requests_count: 0,
            download_delay: 0.0,
            start_time: None,
            end_time: None,
            response_status_count: HashMap::new(),
            domains_response_bytes: HashMap::new(),
            sessions_requests_count: HashMap::new(),
            custom_stats: HashMap::new(),
        }
    }
}

impl CrawlStats {
    pub fn increment_requests_count(&mut self) {
        self.requests_count += 1;
    }

    pub fn increment_status(&mut self, status: u16) {
        *self.response_status_count.entry(status).or_insert(0) += 1;
    }

    pub fn increment_response_bytes(&mut self, bytes: u64) {
        self.response_bytes += bytes;
    }

    pub fn elapsed_seconds(&self) -> f64 {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start).as_secs_f64(),
            (Some(start), None) => start.elapsed().as_secs_f64(),
            _ => 0.0,
        }
    }

    pub fn requests_per_second(&self) -> f64 {
        let elapsed = self.elapsed_seconds();
        if elapsed > 0.0 {
            self.requests_count as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Result of a crawl run.
#[derive(Debug)]
pub struct CrawlResult {
    pub stats: CrawlStats,
    pub items: ItemList,
    pub paused: bool,
}

impl CrawlResult {
    pub fn completed(&self) -> bool {
        !self.paused
    }
}
