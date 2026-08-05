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

// ── Concurrency / throttling behavior ──

/// Spider with a configurable download delay, serving from cache.
struct DelayedCacheSpider {
    urls: Vec<String>,
    delay: f64,
}

#[async_trait]
impl Spider for DelayedCacheSpider {
    fn name(&self) -> &str {
        "delayed-cache-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        self.urls.clone()
    }
    fn download_delay(&self) -> f64 {
        self.delay
    }
    fn development_mode(&self) -> bool {
        true
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        (vec![serde_json::json!({ "url": response.url() })], vec![])
    }
}

#[tokio::test]
async fn cached_responses_skip_the_download_delay() {
    // Regression: the dispatch loop used to sleep download_delay for every
    // request, including ones served from the dev cache — making cached
    // replays as slow as live crawls for no reason. With 4 cached URLs and a
    // 1s delay, the old behavior takes >= 4s; the fix should finish almost
    // instantly.
    let dir = tempfile::tempdir().unwrap();
    let urls: Vec<String> = (0..4)
        .map(|i| format!("https://example.com/fast/{}", i))
        .collect();
    seed_cache(dir.path().to_str().unwrap(), &urls).await;

    let spider = Arc::new(DelayedCacheSpider {
        urls: urls.clone(),
        delay: 1.0,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(dir.path().to_str().unwrap()),
    )
    .expect("engine builds");

    let started = std::time::Instant::now();
    let result = engine.crawl().await;
    let elapsed = started.elapsed();

    assert_eq!(result.items.len(), urls.len());
    assert_eq!(result.stats.cache_hits, urls.len() as u64);
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "cached replay must not sleep the download delay per request; took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn unreadable_cache_entry_still_applies_download_delay() {
    // The dispatch loop skips the delay when a cache entry EXISTS — but if
    // that entry cannot be parsed, the request falls through to a live
    // fetch. The politeness delay must then be applied in the miss path,
    // otherwise corrupt entries would let the crawl hit the site undelayed.
    let (addr, _max, server) = spawn_counting_server(vec![std::time::Duration::ZERO]).await;
    let url = format!("http://{}/page", addr);

    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();

    // Seed a valid entry, then corrupt the file in place (the cache's
    // filename is an internal hash, so locate the single entry on disk).
    seed_cache(&base, std::slice::from_ref(&url)).await;
    let cache_dir = format!("{}/cache", base);
    let entry = std::fs::read_dir(&cache_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|x| x == "json"))
        .expect("seeded cache entry exists");
    std::fs::write(entry.path(), "definitely not json").unwrap();

    let spider = Arc::new(DelayedCacheSpider {
        urls: vec![url],
        delay: 1.0,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");

    let started = std::time::Instant::now();
    let result = engine.crawl().await;
    let elapsed = started.elapsed();

    tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("live fetch must reach the server")
        .unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.stats.cache_misses, 1, "corrupt entry counts as miss");
    assert!(
        elapsed.as_secs_f64() >= 1.0,
        "the owed politeness delay must be applied before the live fetch; \
         took only {:?}",
        elapsed
    );
}

/// Minimal HTTP/1.1 server on a local port that tracks how many requests it
/// is serving simultaneously. `holds` gives the per-request sleep before
/// responding (indexed by accept order; one entry per expected request), so
/// tests can make overlaps observable — or hold only the first request.
/// Returns (addr, max_concurrency_handle, join_handle).
async fn spawn_counting_server(
    holds: Vec<std::time::Duration>,
) -> (
    std::net::SocketAddr,
    Arc<std::sync::atomic::AtomicUsize>,
    tokio::task::JoinHandle<()>,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));
    let max_out = max_seen.clone();

    let handle = tokio::spawn(async move {
        for hold in holds {
            let (mut socket, _) = listener.accept().await.unwrap();
            let current = current.clone();
            let max_seen = max_seen.clone();
            tokio::spawn(async move {
                let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);

                // Read the request head.
                let mut buf = [0u8; 4096];
                let mut head = Vec::new();
                loop {
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    head.extend_from_slice(&buf[..n]);
                    if head.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }

                // Hold the connection open so concurrent requests overlap
                // measurably. Decrement BEFORE writing the response: the
                // client can only issue its next request after receiving
                // these bytes, so a genuine cap violation still shows up as
                // overlap during the hold window, while scheduling jitter
                // after the response can't inflate the counter spuriously.
                tokio::time::sleep(hold).await;
                current.fetch_sub(1, Ordering::SeqCst);
                let body = "<html><body>ok</body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    (addr, max_out, handle)
}

/// Spider hitting a live local server with a per-domain concurrency cap.
struct LiveSpider {
    urls: Vec<String>,
    per_domain: u32,
}

#[async_trait]
impl Spider for LiveSpider {
    fn name(&self) -> &str {
        "live-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        self.urls.clone()
    }
    fn concurrent_requests(&self) -> u32 {
        4
    }
    fn concurrent_requests_per_domain(&self) -> u32 {
        self.per_domain
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        (vec![serde_json::json!({ "url": response.url() })], vec![])
    }
}

#[tokio::test]
async fn per_domain_cap_still_enforced_with_in_task_acquisition() {
    // The per-domain semaphore moved from the dispatch loop into the spawned
    // task (to fix head-of-line blocking). This must NOT weaken the cap: with
    // per_domain=1 and 3 requests to one host, the server must never observe
    // two requests in flight at once.
    let hold = std::time::Duration::from_millis(150);
    let (addr, max_concurrent, server) = spawn_counting_server(vec![hold; 3]).await;

    let urls: Vec<String> = (0..3)
        .map(|i| format!("http://{}/page/{}", addr, i))
        .collect();
    let spider = Arc::new(LiveSpider {
        urls: urls.clone(),
        per_domain: 1,
    });
    let engine = CrawlerEngine::new(spider, SessionManager::new(FetcherConfig::default()), None)
        .expect("engine builds");

    let result = engine.crawl().await;
    // Timeout so a missing request fails the test crisply instead of
    // hanging the whole CI job on a join that never completes.
    tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("server must see all expected requests")
        .unwrap();

    assert_eq!(result.items.len(), urls.len(), "all pages fetched");
    assert_eq!(
        max_concurrent.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "per-domain cap of 1 must never be exceeded"
    );
}

/// Spider over two hosts with explicit priorities: all `high` URLs are
/// dequeued before the single `low` URL, per_domain = 1.
struct TwoHostSpider {
    high: Vec<String>,
    low: String,
}

#[async_trait]
impl Spider for TwoHostSpider {
    fn name(&self) -> &str {
        "two-host-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        let mut urls = self.high.clone();
        urls.push(self.low.clone());
        urls
    }
    fn start_requests(&self) -> Vec<SpiderRequest> {
        let mut reqs: Vec<SpiderRequest> = self
            .high
            .iter()
            .map(|u| SpiderRequest::builder(u).priority(10).build())
            .collect();
        reqs.push(SpiderRequest::builder(&self.low).priority(1).build());
        reqs
    }
    fn concurrent_requests(&self) -> u32 {
        4
    }
    fn concurrent_requests_per_domain(&self) -> u32 {
        1
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        (vec![serde_json::json!({ "url": response.url() })], vec![])
    }
}

#[tokio::test]
async fn saturated_domain_does_not_stall_dispatch_of_other_domains() {
    // Regression for head-of-line blocking: the dispatch loop used to AWAIT
    // the per-domain permit before spawning. With per_domain=1 and two
    // same-domain requests queued first (higher priority), dispatch of every
    // other domain stalled until the first domain's request finished. The
    // permit is now acquired inside the spawned task, so the other domain's
    // request must go out while the first domain is still busy.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // Host A: 127.0.0.1, cap 1. Only the FIRST request is held (3s); the
    // second answers instantly. The old buggy code stalled dispatch during
    // the first hold either way, so discrimination is preserved — but the
    // pass margin for the fixed code grows to ~2.9s, absorbing cold-runner
    // one-time costs (TLS/cert init, localhost::1 fallback, scheduler
    // jitter) on slow CI machines without lengthening the test.
    let hold = std::time::Duration::from_millis(3000);
    let (addr_a, _max_a, server_a) =
        spawn_counting_server(vec![hold, std::time::Duration::ZERO]).await;

    // Host B: localhost (a distinct host string, same loopback), instant
    // response; records when its request arrives.
    let listener_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_b = listener_b.local_addr().unwrap().port();
    let b_received: Arc<std::sync::Mutex<Option<std::time::Instant>>> =
        Arc::new(std::sync::Mutex::new(None));
    let b_received_writer = b_received.clone();
    let server_b = tokio::spawn(async move {
        let (mut socket, _) = listener_b.accept().await.unwrap();
        *b_received_writer.lock().unwrap() = Some(std::time::Instant::now());
        let mut buf = [0u8; 4096];
        let mut head = Vec::new();
        loop {
            let n = socket.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            head.extend_from_slice(&buf[..n]);
            if head.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let body = "<html><body>b</body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    let spider = Arc::new(TwoHostSpider {
        high: (0..2)
            .map(|i| format!("http://{}/slow/{}", addr_a, i))
            .collect(),
        low: format!("http://localhost:{}/fast", port_b),
    });
    let engine = CrawlerEngine::new(spider, SessionManager::new(FetcherConfig::default()), None)
        .expect("engine builds");

    let crawl_start = std::time::Instant::now();
    let result = engine.crawl().await;
    // Timeouts so a missing request fails crisply instead of hanging the job.
    tokio::time::timeout(std::time::Duration::from_secs(30), server_a)
        .await
        .expect("host A must see both requests")
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), server_b)
        .await
        .expect("host B must see its request")
        .unwrap();

    assert_eq!(result.items.len(), 3, "all three pages fetched");
    let b_at = b_received
        .lock()
        .unwrap()
        .expect("host B must have been requested")
        .duration_since(crawl_start);
    assert!(
        b_at < hold,
        "host B must be fetched while host A is still holding its first \
         request (head-of-line blocking); B was only requested after {:?}",
        b_at
    );
}

/// Serve robots.txt and pages over plain HTTP on a local port, so the test
/// proves robots.txt is fetched from the request's ACTUAL scheme and port.
/// Handles `count` sequential connections, routing by path.
async fn spawn_robots_server(
    robots_body: &'static str,
    count: usize,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let mut paths = Vec::new();
        for _ in 0..count {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            loop {
                let n = socket.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let head_text = String::from_utf8_lossy(&head);
            let path = head_text
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_string();
            let (content_type, body) = if path == "/robots.txt" {
                ("text/plain", robots_body.to_string())
            } else {
                ("text/html", format!("<html><body>{}</body></html>", path))
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                content_type,
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
            paths.push(path);
        }
        paths
    });

    (addr, handle)
}

/// Spider that obeys robots.txt against a live local server.
struct RobotsSpider {
    urls: Vec<String>,
}

#[async_trait]
impl Spider for RobotsSpider {
    fn name(&self) -> &str {
        "robots-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        self.urls.clone()
    }
    fn robots_txt_obey(&self) -> bool {
        true
    }
    fn concurrent_requests(&self) -> u32 {
        1
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        (vec![serde_json::json!({ "url": response.url() })], vec![])
    }
}

#[tokio::test]
async fn robots_txt_fetched_from_actual_scheme_and_port_and_enforced() {
    // Regression: robots.txt used to be fetched from a hardcoded
    // https://host/robots.txt — for an HTTP server on a non-default port
    // that fetch failed silently and EVERYTHING was allowed. Now the fetch
    // uses the request's real origin, so this server's Disallow must be
    // honored, including an Allow override and a wildcard pattern.
    let robots = "User-agent: *\nDisallow: /private\nAllow: /private/ok\nDisallow: /*.pdf$\n";
    // Expected connections: robots.txt (seed prefetch) + the 2 allowed pages.
    let (addr, server) = spawn_robots_server(robots, 3).await;

    let urls = vec![
        format!("http://{}/public", addr),
        format!("http://{}/private/secret", addr), // disallowed
        format!("http://{}/private/ok/page", addr), // allowed via Allow override
        format!("http://{}/report.pdf", addr),     // disallowed by wildcard
    ];
    let spider = Arc::new(RobotsSpider { urls });
    let engine = CrawlerEngine::new(spider, SessionManager::new(FetcherConfig::default()), None)
        .expect("engine builds");

    let result = engine.crawl().await;
    let served = tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("server must see robots.txt and the allowed pages")
        .unwrap();

    assert_eq!(
        result.stats.robots_disallowed_count, 2,
        "/private/secret and /report.pdf must be blocked by robots.txt"
    );
    assert_eq!(result.items.len(), 2, "only the allowed pages yield items");
    assert!(served.contains(&"/robots.txt".to_string()));
    assert!(!served.contains(&"/private/secret".to_string()));
    assert!(!served.contains(&"/report.pdf".to_string()));
}

#[tokio::test]
async fn blocked_responses_are_not_cached_in_dev_mode() {
    // Regression: the dev cache used to store the response BEFORE the
    // is_blocked check, so a 403 got cached and every re-enqueued retry was
    // served the same blocked page from disk — the retry loop was defeated
    // permanently. Blocked responses must never be cached: all retries go to
    // the live server, and afterwards the cache must not contain the URL.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // 1 initial + 3 retries (default max_blocked_retries) = 4 live hits.
    let expected_hits = 4usize;
    let server = tokio::spawn(async move {
        let mut hits = 0usize;
        for _ in 0..expected_hits {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            loop {
                let n = socket.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let body = "blocked";
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
            hits += 1;
        }
        hits
    });

    let url = format!("http://{}/always-403", addr);
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();

    let spider = Arc::new(DelayedCacheSpider {
        urls: vec![url.clone()],
        delay: 0.0,
    });
    let engine = CrawlerEngine::new(
        spider,
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");

    let result = engine.crawl().await;
    let hits = tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("every retry must hit the live server, not the cache")
        .unwrap();

    assert_eq!(hits, expected_hits);
    assert_eq!(result.stats.blocked_requests_count, expected_hits as u64);
    assert_eq!(result.items.len(), 0);
    let cache = ResponseCache::new(&format!("{}/cache", base)).unwrap();
    assert!(
        cache.get(&url).await.is_none(),
        "a blocked response must never be written to the dev cache"
    );
}

/// Spider with AutoThrottle enabled against a live local server.
struct ThrottledSpider {
    urls: Vec<String>,
}

#[async_trait]
impl Spider for ThrottledSpider {
    fn name(&self) -> &str {
        "throttled-spider"
    }
    fn start_urls(&self) -> Vec<String> {
        self.urls.clone()
    }
    fn autothrottle_enabled(&self) -> bool {
        true
    }
    fn autothrottle_start_delay(&self) -> f64 {
        0.05 // fast until the server pushes back
    }
    fn max_blocked_retries(&self) -> u32 {
        1
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        (vec![serde_json::json!({ "url": response.url() })], vec![])
    }
}

#[tokio::test]
async fn autothrottle_backs_off_on_retry_after_before_retrying() {
    // First response is a 429 with Retry-After: 1. The 429 is blocked ->
    // re-enqueued once (max_blocked_retries = 1). AutoThrottle must push the
    // domain's schedule out, so the retry reaches the server >= ~1s after
    // the first request, even though the start delay is only 50ms. The
    // second response is a 200 and the crawl completes.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut request_times = Vec::new();
        for i in 0..2usize {
            let (mut socket, _) = listener.accept().await.unwrap();
            request_times.push(std::time::Instant::now());
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            loop {
                let n = socket.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let response = if i == 0 {
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Type: text/html\r\nContent-Length: 7\r\nConnection: close\r\n\r\nslow it".to_string()
            } else {
                let body = "<html><body>ok</body></html>";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            };
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
        request_times
    });

    let url = format!("http://{}/throttled", addr);
    let spider = Arc::new(ThrottledSpider { urls: vec![url] });
    let engine = CrawlerEngine::new(spider, SessionManager::new(FetcherConfig::default()), None)
        .expect("engine builds");

    let result = engine.crawl().await;
    let times = tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("server must see the original request and the retry")
        .unwrap();

    assert_eq!(result.items.len(), 1, "retry must eventually succeed");
    assert_eq!(result.stats.blocked_requests_count, 1);
    let gap = times[1].duration_since(times[0]);
    assert!(
        gap.as_secs_f64() >= 0.9,
        "retry must wait ~Retry-After (1s) after the 429; gap was {:?}",
        gap
    );
    assert!(
        gap.as_secs_f64() < 10.0,
        "back-off must not balloon; gap was {:?}",
        gap
    );
}

#[tokio::test]
async fn per_domain_and_per_session_stats_are_populated() {
    // domains_response_bytes and sessions_requests_count were public fields
    // that nothing ever wrote; the engine now populates them on every live
    // response.
    let (addr, _max, server) = spawn_counting_server(vec![std::time::Duration::ZERO; 2]).await;
    let urls: Vec<String> = (0..2)
        .map(|i| format!("http://{}/stats/{}", addr, i))
        .collect();
    let spider = Arc::new(LiveSpider {
        urls: urls.clone(),
        per_domain: 0,
    });
    let engine = CrawlerEngine::new(spider, SessionManager::new(FetcherConfig::default()), None)
        .expect("engine builds");

    let result = engine.crawl().await;
    tokio::time::timeout(std::time::Duration::from_secs(30), server)
        .await
        .expect("server must see both requests")
        .unwrap();

    assert_eq!(result.items.len(), 2);
    let bytes = result
        .stats
        .domains_response_bytes
        .get("127.0.0.1")
        .copied()
        .unwrap_or(0);
    assert!(bytes > 0, "per-domain byte count must be populated");
    assert_eq!(
        result.stats.sessions_requests_count.get("default").copied(),
        Some(2),
        "requests without a session id count under \"default\""
    );
}
