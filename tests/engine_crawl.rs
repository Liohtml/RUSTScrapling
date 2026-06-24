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
