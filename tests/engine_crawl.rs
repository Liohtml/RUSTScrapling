//! Async behavioral tests for `CrawlerEngine::crawl` (#39).
//!
//! These exercise the real async crawl loop end-to-end without network I/O by
//! running in `development_mode` with a pre-seeded response cache, so every
//! request resolves from disk instead of hitting the network.

use async_trait::async_trait;
use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::spiders::cache::{CachedResponse, ResponseCache};
use rust_scrapling::spiders::engine::CrawlerEngine;
use rust_scrapling::spiders::request::SpiderRequest;
use rust_scrapling::spiders::response::SpiderResponse;
use rust_scrapling::spiders::session::SessionManager;
use rust_scrapling::spiders::spider::Spider;
use std::collections::HashMap;
use std::sync::Arc;

struct CacheSpider {
    urls: Vec<String>,
    concurrent: u32,
}

#[async_trait]
impl Spider for CacheSpider {
    fn name(&self) -> &str {
        "cache-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        self.urls.clone()
    }
    fn concurrent_requests(&self) -> u32 {
        self.concurrent
    }
    fn development_mode(&self) -> bool {
        true
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        // Emit one item per page, no follow-up requests.
        let item = serde_json::json!({ "url": response.url() });
        (vec![item], vec![])
    }
}

async fn seed_cache(dir: &str, urls: &[String]) {
    let cache = ResponseCache::new(&format!("{}/cache", dir)).unwrap();
    for url in urls {
        let cached = CachedResponse {
            status: 200,
            content_type: "text/html".to_string(),
            body: format!("<html><body>{}</body></html>", url),
            url: url.clone(),
            headers: HashMap::new(),
        };
        cache.put(url, &cached).await.unwrap();
    }
}

#[tokio::test]
async fn crawl_processes_all_urls_from_cache_and_terminates() {
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<String> = (0..5)
        .map(|i| format!("https://example.com/{}", i))
        .collect();
    seed_cache(dir.path().to_str().unwrap(), &urls).await;

    let spider = Arc::new(CacheSpider {
        urls: urls.clone(),
        concurrent: 4,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(dir.path().to_str().unwrap()),
    )
    .expect("engine builds");

    // If the loop did not terminate, the test would hang.
    let result = engine.crawl().await;

    assert!(!result.paused);
    assert_eq!(result.items.len(), urls.len());
    assert_eq!(result.stats.cache_hits, urls.len() as u64);
}

#[tokio::test]
async fn pause_before_crawl_stops_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<String> = (0..3)
        .map(|i| format!("https://example.com/p/{}", i))
        .collect();
    seed_cache(dir.path().to_str().unwrap(), &urls).await;

    let spider = Arc::new(CacheSpider {
        urls,
        concurrent: 2,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(dir.path().to_str().unwrap()),
    )
    .expect("engine builds");

    // Request pause before starting: the loop should break on its first check.
    engine.request_pause();
    let result = engine.crawl().await;

    assert!(result.paused, "crawl should report paused");
    assert_eq!(
        result.items.len(),
        0,
        "no items should be processed once paused"
    );
}

#[tokio::test]
async fn crawl_with_single_concurrency_still_completes() {
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<String> = (0..6)
        .map(|i| format!("https://example.com/s/{}", i))
        .collect();
    seed_cache(dir.path().to_str().unwrap(), &urls).await;

    let spider = Arc::new(CacheSpider {
        urls: urls.clone(),
        concurrent: 1,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(dir.path().to_str().unwrap()),
    )
    .expect("engine builds");

    let result = engine.crawl().await;
    assert_eq!(result.items.len(), urls.len());
    assert_eq!(result.stats.concurrent_requests, 1);
}

/// Spider that follows a link from each fetched page, used to verify that
/// the seen set restored from a checkpoint filters already-crawled URLs.
struct FollowSpider {
    start: String,
    follow: String,
}

#[async_trait]
impl Spider for FollowSpider {
    fn name(&self) -> &str {
        "follow-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        vec![self.start.clone()]
    }
    fn development_mode(&self) -> bool {
        true
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let item = serde_json::json!({ "url": response.url() });
        (vec![item], vec![SpiderRequest::new(&self.follow)])
    }
}

#[tokio::test]
async fn resume_restores_seen_set_and_skips_already_crawled_urls() {
    use rust_scrapling::spiders::checkpoint::{CheckpointData, CheckpointManager};

    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    let start = "https://example.com/start".to_string();
    let follow = "https://example.com/already-crawled".to_string();
    seed_cache(&base, &[start.clone(), follow.clone()]).await;

    // Simulate a paused crawl that already visited `follow`: its fingerprint
    // is in the checkpoint's seen set, and `start` is still pending.
    let follow_fp = SpiderRequest::new(&follow).fingerprint().to_string();
    let mgr = CheckpointManager::new(&format!("{}/checkpoints", base)).unwrap();
    mgr.save(&CheckpointData {
        pending_requests: vec![SpiderRequest::new(&start)],
        pending_urls: vec![],
        seen_fingerprints: vec![follow_fp],
        items_count: 0,
    })
    .await
    .unwrap();

    let spider = Arc::new(FollowSpider {
        start: start.clone(),
        follow,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;

    // Only the pending start URL is processed; the follow link it emits is
    // filtered by the restored seen set instead of being crawled again.
    assert!(!result.paused);
    assert_eq!(result.items.len(), 1, "follow URL must not be re-crawled");
    let items: Vec<serde_json::Value> = result.items.into_iter().collect();
    assert_eq!(items[0]["url"], serde_json::json!(start));
}

#[tokio::test]
async fn checkpoint_saved_on_pause_includes_seen_fingerprints() {
    use rust_scrapling::spiders::checkpoint::CheckpointManager;

    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    let urls: Vec<String> = (0..3)
        .map(|i| format!("https://example.com/cp/{}", i))
        .collect();
    seed_cache(&base, &urls).await;

    let spider = Arc::new(CacheSpider {
        urls: urls.clone(),
        concurrent: 2,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");

    // Pause immediately: all start requests stay pending, and every enqueued
    // URL already has its fingerprint in the seen set.
    engine.request_pause();
    let result = engine.crawl().await;
    assert!(result.paused);

    let mgr = CheckpointManager::new(&format!("{}/checkpoints", base)).unwrap();
    let data = mgr.restore().await.expect("checkpoint written on pause");
    assert_eq!(data.pending_requests.len(), urls.len());
    assert_eq!(
        data.seen_fingerprints.len(),
        urls.len(),
        "seen set must be persisted, not saved as empty"
    );
}
