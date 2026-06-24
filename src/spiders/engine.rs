use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use url::Url;

use crate::fetchers::response::Response as FetcherResponse;
use crate::spiders::cache::{CachedResponse, ResponseCache};
use crate::spiders::checkpoint::{CheckpointData, CheckpointManager};
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::result::{CrawlResult, CrawlStats, ItemList};
use crate::spiders::robots::RobotsTxtManager;
use crate::spiders::scheduler::Scheduler;
use crate::spiders::session::SessionManager;
use crate::spiders::spider::Spider;

/// Sanitize a string for safe use as a single filesystem path segment.
/// Replaces any character that is not alphanumeric, `_`, or `-` with `_` so
/// values like `"../etc"` cannot escape the intended directory.
fn sanitize_path_segment(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "_".to_string()
    } else {
        cleaned
    }
}

/// Decrements the active-task counter on drop. Using a guard (rather than an
/// explicit `fetch_sub` after the `.await`) guarantees the counter is balanced
/// even if `process_request` panics — otherwise the panicking task would leak
/// a count and the crawl loop, which exits on `active_tasks == 0`, would hang
/// forever.
struct ActiveTaskGuard {
    active_tasks: Arc<AtomicU32>,
}

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        self.active_tasks.fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct CrawlerEngine<S: Spider> {
    spider: Arc<S>,
    session_manager: Arc<SessionManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    stats: Arc<Mutex<CrawlStats>>,
    items: Arc<Mutex<ItemList>>,
    global_limiter: Arc<Semaphore>,
    /// Per-domain semaphores, lazily created on first request to each host.
    /// Used to enforce `Spider::concurrent_requests_per_domain`.
    domain_limiters: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    robots_manager: Option<Arc<Mutex<RobotsTxtManager>>>,
    cache: Option<Arc<ResponseCache>>,
    checkpoint: Option<Arc<CheckpointManager>>,
    paused: Arc<AtomicBool>,
    active_tasks: Arc<AtomicU32>,
}

impl<S: Spider> CrawlerEngine<S> {
    pub fn new(
        spider: Arc<S>,
        mut session_manager: SessionManager,
        crawl_dir: Option<&str>,
    ) -> Self {
        session_manager.ensure_default();

        let concurrent = spider.concurrent_requests().max(1);

        let robots_manager = if spider.robots_txt_obey() {
            Some(Arc::new(Mutex::new(RobotsTxtManager::new("RUSTScrapling"))))
        } else {
            None
        };

        let cache = if spider.development_mode() {
            let dir = crawl_dir
                .map(|d| format!("{}/cache", d))
                .unwrap_or_else(|| {
                    format!(".scrapling/{}/cache", sanitize_path_segment(spider.name()))
                });
            ResponseCache::new(&dir).ok().map(Arc::new)
        } else {
            None
        };

        let checkpoint = {
            let dir = crawl_dir
                .map(|d| format!("{}/checkpoints", d))
                .unwrap_or_else(|| {
                    format!(
                        ".scrapling/{}/checkpoints",
                        sanitize_path_segment(spider.name())
                    )
                });
            CheckpointManager::new(&dir).ok().map(Arc::new)
        };

        let scheduler = Scheduler::new(
            spider.fp_include_kwargs(),
            spider.fp_include_headers(),
            spider.fp_keep_fragments(),
        );

        Self {
            spider,
            session_manager: Arc::new(session_manager),
            scheduler: Arc::new(Mutex::new(scheduler)),
            stats: Arc::new(Mutex::new(CrawlStats::default())),
            items: Arc::new(Mutex::new(ItemList::new())),
            global_limiter: Arc::new(Semaphore::new(concurrent as usize)),
            domain_limiters: Arc::new(Mutex::new(HashMap::new())),
            robots_manager,
            cache,
            checkpoint,
            paused: Arc::new(AtomicBool::new(false)),
            active_tasks: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn request_pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub async fn crawl(self) -> CrawlResult {
        // Set start time and config in stats
        {
            let mut stats = self.stats.lock().await;
            stats.start_time = Some(Instant::now());
            stats.concurrent_requests = self.spider.concurrent_requests();
            stats.concurrent_requests_per_domain = self.spider.concurrent_requests_per_domain();
            stats.download_delay = self.spider.download_delay();
        }

        // Check for checkpoint restore
        let resuming = if let Some(ref cp) = self.checkpoint {
            if cp.exists() {
                if let Some(data) = cp.restore().await {
                    let mut sched = self.scheduler.lock().await;
                    for url in &data.pending_urls {
                        let mut req = SpiderRequest::new(url);
                        req.set_dont_filter(true);
                        sched.enqueue(req);
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Call spider.on_start
        self.spider.on_start(resuming).await;

        // Pre-fetch robots.txt for seed domains
        if let Some(ref robots) = self.robots_manager {
            let start_urls = self.spider.start_urls();
            let mut domains_seen = std::collections::HashSet::new();
            for url in &start_urls {
                if let Some(domain) = Url::parse(url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                {
                    if domains_seen.insert(domain.clone()) {
                        robots.lock().await.fetch_robots(&domain).await;
                    }
                }
            }
        }

        // Enqueue start requests (unless resuming)
        if !resuming {
            let requests = self.spider.start_requests();
            let mut sched = self.scheduler.lock().await;
            for req in requests {
                sched.enqueue(req);
            }
        }

        // Main crawl loop
        loop {
            if self.paused.load(Ordering::SeqCst) {
                break;
            }

            let request = {
                let mut sched = self.scheduler.lock().await;
                sched.dequeue()
            };

            match request {
                Some(req) => {
                    // Apply the spider's download delay here, in the single
                    // dispatch loop, so it actually throttles request *rate*.
                    // (Applying it inside each spawned task only added latency:
                    // N concurrent tasks slept in parallel and then all fired
                    // at once.)
                    let delay = self.spider.download_delay();
                    if delay > 0.0 {
                        tokio::time::sleep(Duration::from_secs_f64(delay)).await;
                    }

                    let permit = self.global_limiter.clone().acquire_owned().await;
                    if permit.is_err() {
                        break;
                    }
                    let permit = permit.unwrap();

                    // Acquire a per-domain permit when a per-domain cap is
                    // configured, so a single host cannot exceed it.
                    let per_domain = self.spider.concurrent_requests_per_domain();
                    let domain_permit = if per_domain > 0 {
                        let domain = req.domain().unwrap_or_default();
                        let sem = {
                            let mut limiters = self.domain_limiters.lock().await;
                            limiters
                                .entry(domain)
                                .or_insert_with(|| Arc::new(Semaphore::new(per_domain as usize)))
                                .clone()
                        };
                        sem.acquire_owned().await.ok()
                    } else {
                        None
                    };

                    self.active_tasks.fetch_add(1, Ordering::SeqCst);

                    let spider = self.spider.clone();
                    let session_manager = self.session_manager.clone();
                    let scheduler = self.scheduler.clone();
                    let stats = self.stats.clone();
                    let items = self.items.clone();
                    let robots_manager = self.robots_manager.clone();
                    let cache = self.cache.clone();
                    let active_tasks = self.active_tasks.clone();

                    tokio::spawn(async move {
                        // These all release on drop — including during a panic
                        // unwind — so the counter stays balanced and the
                        // permits are returned even if `process_request` panics.
                        let _task_guard = ActiveTaskGuard { active_tasks };
                        let _permit = permit;
                        let _domain_permit = domain_permit;

                        Self::process_request(
                            spider,
                            session_manager,
                            scheduler,
                            stats,
                            items,
                            robots_manager,
                            cache,
                            req,
                        )
                        .await;
                    });
                }
                None => {
                    // No requests in queue - check if tasks are still running
                    if self.active_tasks.load(Ordering::SeqCst) == 0 {
                        break;
                    }
                    // Wait a bit for tasks to produce new requests
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }

        // On pause, save checkpoint — but first wait for in-flight tasks to
        // finish so any URLs they enqueue are included in the persisted state.
        let was_paused = self.paused.load(Ordering::SeqCst);
        if was_paused {
            while self.active_tasks.load(Ordering::SeqCst) > 0 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if let Some(ref cp) = self.checkpoint {
                let sched = self.scheduler.lock().await;
                let pending_urls: Vec<String> = sched
                    .pending_requests()
                    .iter()
                    .map(|r| r.url().to_string())
                    .collect();
                let items_count = self.items.lock().await.len() as u64;
                let data = CheckpointData {
                    pending_urls,
                    seen_fingerprints: Vec::new(), // Fingerprints are internal to scheduler
                    items_count,
                };
                let _ = cp.save(&data).await;
            }
        } else {
            // Clean up checkpoint on successful completion
            if let Some(ref cp) = self.checkpoint {
                cp.cleanup().await;
            }
        }

        // Set end time
        {
            let mut stats = self.stats.lock().await;
            stats.end_time = Some(Instant::now());
        }

        // Call spider.on_close
        self.spider.on_close().await;

        // Build result - extract inner values from Arc<Mutex<>>
        let stats = self.stats.lock().await.clone();
        let items = self.items.lock().await.clone();

        CrawlResult {
            stats,
            items,
            paused: was_paused,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn process_request(
        spider: Arc<S>,
        session_manager: Arc<SessionManager>,
        scheduler: Arc<Mutex<Scheduler>>,
        stats: Arc<Mutex<CrawlStats>>,
        items: Arc<Mutex<ItemList>>,
        robots_manager: Option<Arc<Mutex<RobotsTxtManager>>>,
        cache: Option<Arc<ResponseCache>>,
        request: SpiderRequest,
    ) {
        let url = request.url().to_string();

        // Check robots.txt. Lazily fetch robots.txt for domains discovered
        // mid-crawl (only seed domains are pre-fetched), otherwise a follow
        // link to a new host would bypass the Robots Exclusion Protocol.
        let mut robots_crawl_delay: Option<f64> = None;
        if let Some(ref robots) = robots_manager {
            let domain = request.domain().unwrap_or_default();
            let mut robots = robots.lock().await;
            if !domain.is_empty() && !robots.has_domain(&domain) {
                robots.fetch_robots(&domain).await;
            }
            if !robots.is_allowed(&url) {
                stats.lock().await.robots_disallowed_count += 1;
                return;
            }
            // Honor the site's Crawl-delay directive for this domain.
            robots_crawl_delay = robots.crawl_delay(&domain);
        }

        // Check allowed_domains. Reject requests whose domain cannot be
        // parsed (e.g. `data:`, `file://`, malformed URLs) so the whitelist
        // cannot be bypassed by non-HTTP schemes.
        let allowed = spider.allowed_domains();
        if !allowed.is_empty() {
            match request.domain() {
                Some(domain) if allowed.contains(&domain) => {}
                _ => {
                    stats.lock().await.offsite_requests_count += 1;
                    return;
                }
            }
        }

        // Check dev cache
        if let Some(ref response_cache) = cache {
            if let Some(cached) = response_cache.get(&url).await {
                stats.lock().await.cache_hits += 1;

                let fetcher_resp = FetcherResponse::new(
                    cached.status,
                    cached.content_type.clone(),
                    cached.body.clone(),
                    cached.url.clone(),
                    cached.headers.clone(),
                );
                let spider_resp = SpiderResponse::new(fetcher_resp);

                // Parse and collect items
                let (parsed_items, follow_requests) = spider.parse(spider_resp).await;
                Self::collect_items(&spider, &items, &stats, parsed_items).await;
                Self::enqueue_follow_requests(&scheduler, follow_requests).await;
                return;
            } else {
                stats.lock().await.cache_misses += 1;
            }
        }

        // The spider's static download_delay is applied in the dispatch loop
        // (so it throttles rate). Here we additionally honor the per-domain
        // Crawl-delay parsed from this host's robots.txt.
        if let Some(d) = robots_crawl_delay {
            if d > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(d)).await;
            }
        }

        // Fetch via session_manager
        let result = session_manager.fetch(&request).await;

        match result {
            Ok(response) => {
                let status = response.status();
                let content_len = response.content_length() as u64;

                // Update stats
                {
                    let mut s = stats.lock().await;
                    s.increment_requests_count();
                    s.increment_status(status);
                    s.increment_response_bytes(content_len);
                }

                // Store to cache if dev mode
                if let Some(ref response_cache) = cache {
                    let cached = CachedResponse {
                        status: response.status(),
                        content_type: response.content_type().to_string(),
                        body: response.text().to_string(),
                        url: response.url().to_string(),
                        headers: response.headers().clone(),
                    };
                    let _ = response_cache.put(&url, &cached).await;
                }

                let spider_resp = SpiderResponse::new(response);

                // Check if blocked
                if spider.is_blocked(&spider_resp).await {
                    let mut s = stats.lock().await;
                    s.blocked_requests_count += 1;

                    if request.retry_count() < spider.max_blocked_retries() {
                        let mut retry_req = request.copy();
                        retry_req.increment_retry();
                        retry_req.set_dont_filter(true);
                        scheduler.lock().await.enqueue(retry_req);
                    }
                    return;
                }

                // Parse response
                let (parsed_items, follow_requests) = spider.parse(spider_resp).await;
                Self::collect_items(&spider, &items, &stats, parsed_items).await;
                Self::enqueue_follow_requests(&scheduler, follow_requests).await;
            }
            Err(err) => {
                spider.on_error(&request, &err).await;
                stats.lock().await.failed_requests_count += 1;
            }
        }
    }

    async fn collect_items(
        spider: &Arc<S>,
        items: &Arc<Mutex<ItemList>>,
        stats: &Arc<Mutex<CrawlStats>>,
        parsed_items: Vec<serde_json::Value>,
    ) {
        for item in parsed_items {
            match spider.on_scraped_item(item).await {
                Some(processed) => {
                    items.lock().await.push(processed);
                    stats.lock().await.items_scraped += 1;
                }
                None => {
                    stats.lock().await.items_dropped += 1;
                }
            }
        }
    }

    async fn enqueue_follow_requests(
        scheduler: &Arc<Mutex<Scheduler>>,
        follow_requests: Vec<SpiderRequest>,
    ) {
        if !follow_requests.is_empty() {
            let mut sched = scheduler.lock().await;
            for req in follow_requests {
                sched.enqueue(req);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_path_segment_replaces_traversal_chars() {
        assert_eq!(sanitize_path_segment("../etc/passwd"), "___etc_passwd");
        assert_eq!(sanitize_path_segment("..\\windows"), "___windows");
        assert_eq!(sanitize_path_segment("spider.name"), "spider_name");
    }

    #[test]
    fn sanitize_path_segment_keeps_safe_chars() {
        assert_eq!(sanitize_path_segment("my-spider_1"), "my-spider_1");
        assert_eq!(sanitize_path_segment("Spider123"), "Spider123");
    }

    #[test]
    fn sanitize_path_segment_handles_empty() {
        assert_eq!(sanitize_path_segment(""), "_");
        assert_eq!(sanitize_path_segment("///"), "___");
    }
}
