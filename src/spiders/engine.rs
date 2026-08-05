//! The [`CrawlerEngine`]: the async crawl loop that drives a
//! [`Spider`] — scheduling, concurrency limits, robots.txt, throttling,
//! caching, checkpointing, and stats collection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

use crate::fetchers::client::FetcherError;
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
use crate::spiders::throttle::{parse_retry_after, AutoThrottle};

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

/// Runs a [`Spider`] to completion (or until paused).
///
/// The engine dequeues requests by priority and processes them
/// concurrently up to the spider's global and per-domain limits. Each
/// request goes through robots.txt checks (when enabled), the
/// allowed-domains filter, the dev-mode cache, politeness delays (static
/// or AutoThrottle), the session fetch, blocked-response detection with
/// retries, and finally the spider's `parse` callback — whose items are
/// piped through `on_scraped_item` and whose follow-up requests are
/// enqueued (deduplicated).
///
/// Construct with [`CrawlerEngine::new`], run with
/// [`CrawlerEngine::crawl`], and optionally pause a running crawl with
/// [`CrawlerEngine::request_pause`] — a paused crawl writes a checkpoint
/// that the next run restores automatically.
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
    /// AutoThrottle state when `Spider::autothrottle_enabled()`. A std
    /// (not tokio) mutex: every critical section is a short, await-free
    /// computation; the actual sleeping happens after the guard is dropped.
    throttle: Option<Arc<std::sync::Mutex<AutoThrottle>>>,
    paused: Arc<AtomicBool>,
    active_tasks: Arc<AtomicU32>,
}

impl<S: Spider> CrawlerEngine<S> {
    /// Build an engine for `spider`.
    ///
    /// `session_manager` provides the HTTP sessions (its `"default"`
    /// session is created here if missing — the reason this can fail with
    /// [`FetcherError`]). `crawl_dir` overrides where the dev cache and
    /// checkpoints live; `None` uses `.scrapling/<spider name>/`.
    pub fn new(
        spider: Arc<S>,
        mut session_manager: SessionManager,
        crawl_dir: Option<&str>,
    ) -> Result<Self, FetcherError> {
        session_manager.ensure_default()?;

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

        let throttle = if spider.autothrottle_enabled() {
            Some(Arc::new(std::sync::Mutex::new(AutoThrottle::new(
                spider.autothrottle_start_delay(),
                spider.autothrottle_max_delay(),
                spider.autothrottle_target_concurrency(),
            ))))
        } else {
            None
        };

        Ok(Self {
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
            throttle,
            paused: Arc::new(AtomicBool::new(false)),
            active_tasks: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Ask a running crawl to pause. The crawl loop stops dispatching,
    /// waits for in-flight requests to finish, writes a checkpoint
    /// (pending requests plus the dedup set), and returns a
    /// [`CrawlResult`] with `paused == true`. The next
    /// [`CrawlerEngine::crawl`] with the same spider/crawl dir resumes
    /// from that checkpoint.
    pub fn request_pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// Run the crawl to completion (or until paused) and return the
    /// scraped items and stats.
    ///
    /// Restores a checkpoint if one exists (skipping the start requests),
    /// pre-fetches robots.txt for the seed origins when the spider obeys
    /// robots.txt, then drives the crawl loop until the queue is empty and
    /// no tasks are in flight. On normal completion the checkpoint is
    /// cleaned up; on pause it is written (see
    /// [`CrawlerEngine::request_pause`]).
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
                    // Restore the dedup set first so URLs crawled before the
                    // pause are not re-visited. The pending requests below
                    // bypass it explicitly via dont_filter: most of their
                    // fingerprints are in the restored set already, and ones
                    // that are not (originally enqueued with dont_filter,
                    // e.g. blocked retries) must be re-crawled anyway.
                    sched.restore_seen(data.seen_fingerprints);
                    if !data.pending_requests.is_empty() {
                        // Full requests (method, headers, body, meta,
                        // priority) survived the checkpoint — restore them
                        // as-is rather than degrading to bare GETs.
                        for mut req in data.pending_requests {
                            req.set_dont_filter(true);
                            sched.enqueue(req);
                        }
                    } else {
                        // Checkpoint written before `pending_requests`
                        // existed: only bare URLs are available.
                        for url in &data.pending_urls {
                            let mut req = SpiderRequest::new(url);
                            req.set_dont_filter(true);
                            sched.enqueue(req);
                        }
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

        // Pre-fetch robots.txt for seed origins (actual scheme + port, same
        // as the lazy path in process_request).
        if let Some(ref robots) = self.robots_manager {
            let start_urls = self.spider.start_urls();
            let mut origins_seen = std::collections::HashSet::new();
            for url in &start_urls {
                if let Some((origin, key)) = RobotsTxtManager::origin_and_key(url) {
                    if origins_seen.insert(key.clone()) {
                        // Same lock discipline as the lazy path in
                        // process_request: fetch without holding the manager
                        // lock (uncontended here, but consistent).
                        let user_agent = robots.lock().await.user_agent().to_string();
                        let rules = RobotsTxtManager::fetch_rules(&user_agent, &origin).await;
                        robots.lock().await.insert_rules(&key, rules);
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
                    // at once.) Requests that will be served from the dev
                    // cache skip the delay — replays hit disk, not the site,
                    // so there is nothing to be polite to.
                    // With AutoThrottle enabled the static delay is replaced
                    // by the adaptive per-domain schedule inside the task
                    // (download_delay still acts as its floor), so the
                    // dispatch loop never sleeps for one domain while others
                    // could be dispatched.
                    let delay = self.spider.download_delay();
                    let mut delay_was_skipped = false;
                    if delay > 0.0 && self.throttle.is_none() {
                        let served_from_cache = match &self.cache {
                            Some(c) => c.contains(req.url()).await,
                            None => false,
                        };
                        if served_from_cache {
                            // process_request re-applies the delay if the
                            // entry then turns out unreadable and the request
                            // goes to the live site after all.
                            delay_was_skipped = true;
                        } else {
                            tokio::time::sleep(Duration::from_secs_f64(delay)).await;
                        }
                    }

                    let permit = self.global_limiter.clone().acquire_owned().await;
                    if permit.is_err() {
                        break;
                    }
                    let permit = permit.unwrap();

                    // Look up (but do NOT acquire) the per-domain semaphore
                    // here. The acquisition happens inside the spawned task:
                    // awaiting it in this single dispatch loop would let one
                    // saturated domain stall dispatch for every other domain
                    // (head-of-line blocking), degrading the crawl toward
                    // serial whenever the priority queue clusters same-domain
                    // URLs.
                    let per_domain = self.spider.concurrent_requests_per_domain();
                    let domain_sem = if per_domain > 0 {
                        let domain = req.domain().unwrap_or_default();
                        let mut limiters = self.domain_limiters.lock().await;
                        Some(
                            limiters
                                .entry(domain)
                                .or_insert_with(|| Arc::new(Semaphore::new(per_domain as usize)))
                                .clone(),
                        )
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
                    let throttle = self.throttle.clone();
                    let active_tasks = self.active_tasks.clone();

                    tokio::spawn(async move {
                        // These all release on drop — including during a panic
                        // unwind — so the counter stays balanced and the
                        // permits are returned even if `process_request` panics.
                        let _task_guard = ActiveTaskGuard { active_tasks };
                        let _permit = permit;
                        // Waiting for the domain permit happens here, inside
                        // the task. This task holds a global permit while it
                        // waits, but the dispatch loop stays free to spawn
                        // work for other domains up to the global cap.
                        // Residual limitation: N same-domain requests still
                        // soak N global permits while parked here, so a
                        // saturated domain can eventually exhaust the global
                        // cap too — bounded head-of-line, fully fixable only
                        // with per-domain queues (Scrapy downloader-slot
                        // style). On pause, parked tasks drain serially
                        // through the domain permit before the checkpoint is
                        // written (no data loss, just added pause latency).
                        let _domain_permit = match domain_sem {
                            Some(sem) => sem.acquire_owned().await.ok(),
                            None => None,
                        };

                        Self::process_request(
                            spider,
                            session_manager,
                            scheduler,
                            stats,
                            items,
                            robots_manager,
                            cache,
                            throttle,
                            req,
                            delay_was_skipped,
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
                let (pending_requests, seen_fingerprints) = {
                    let sched = self.scheduler.lock().await;
                    sched.snapshot()
                };
                let items_count = self.items.lock().await.len() as u64;
                let data = CheckpointData {
                    pending_requests,
                    pending_urls: Vec::new(),
                    seen_fingerprints,
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
        throttle: Option<Arc<std::sync::Mutex<AutoThrottle>>>,
        request: SpiderRequest,
        // True when the dispatch loop skipped the download delay because
        // this URL appeared to be in the dev cache. If the entry then turns
        // out unreadable (corrupt/deleted between check and read), the miss
        // path below owes the site that delay before fetching live.
        delay_was_skipped: bool,
    ) {
        let url = request.url().to_string();

        // Check robots.txt. Lazily fetch robots.txt for domains discovered
        // mid-crawl (only seed domains are pre-fetched), otherwise a follow
        // link to a new host would bypass the Robots Exclusion Protocol.
        let mut robots_crawl_delay: Option<f64> = None;
        if let Some(ref robots) = robots_manager {
            // Fetch robots.txt from the request's actual origin (scheme +
            // host + explicit port), not a hardcoded https://host/ — an
            // HTTP-only site or a non-default port would otherwise silently
            // become allow-all. URLs without a host skip robots handling.
            if let Some((origin, key)) = RobotsTxtManager::origin_and_key(&url) {
                // The network fetch runs WITHOUT holding the manager lock:
                // holding it across the round-trip (up to the 10s robots
                // timeout) would serialize every other task's is_allowed
                // check behind this origin's fetch.
                let (needs_fetch, user_agent) = {
                    let guard = robots.lock().await;
                    (!guard.has_domain(&key), guard.user_agent().to_string())
                };
                if needs_fetch {
                    let rules = RobotsTxtManager::fetch_rules(&user_agent, &origin).await;
                    let mut guard = robots.lock().await;
                    // Concurrent tasks may race to fetch the same origin;
                    // the first insert wins (they fetched the same content).
                    if !guard.has_domain(&key) {
                        guard.insert_rules(&key, rules);
                    }
                }
                let disallowed = {
                    let guard = robots.lock().await;
                    if guard.is_allowed(&url) {
                        // Honor the site's Crawl-delay directive.
                        robots_crawl_delay = guard.crawl_delay(&key);
                        false
                    } else {
                        true
                    }
                };
                if disallowed {
                    stats.lock().await.robots_disallowed_count += 1;
                    return;
                }
            }
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
                // The dispatch loop skipped the politeness delay because a
                // cache entry existed — but it could not be read (corrupt,
                // schema drift, or deleted since the check), so this request
                // is about to hit the live site after all. Apply the owed
                // delay now; without this, a directory of unreadable entries
                // would let the whole crawl run undelayed.
                if delay_was_skipped {
                    let d = spider.download_delay();
                    if d > 0.0 {
                        tokio::time::sleep(Duration::from_secs_f64(d)).await;
                    }
                }
            }
        }

        let domain = request.domain().unwrap_or_default();
        // The floor no adaptive schedule may undercut: the spider's static
        // download_delay and the site's robots.txt Crawl-delay. Defensively
        // sanitized — Duration::from_secs_f64 panics on non-finite or
        // oversized input, and part of this comes from remote data (the
        // robots parser validates too; this guards the spider-provided
        // side and any future floor source).
        let delay_floor = spider
            .download_delay()
            .max(robots_crawl_delay.unwrap_or(0.0));
        let delay_floor = if delay_floor.is_finite() {
            delay_floor.clamp(0.0, 86_400.0)
        } else {
            0.0
        };

        if let Some(ref throttle) = throttle {
            // AutoThrottle: reserve this domain's next slot (spacing =
            // current adaptive delay, floored) and wait for it. The guard is
            // dropped before sleeping.
            let wait = {
                let mut t = throttle.lock().unwrap_or_else(|p| p.into_inner());
                t.reserve(&domain, delay_floor, tokio::time::Instant::now())
            };
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        } else if let Some(d) = robots_crawl_delay {
            // Without AutoThrottle the static download_delay is applied in
            // the dispatch loop (so it throttles rate); here we additionally
            // honor the per-domain Crawl-delay parsed from robots.txt.
            if d > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(d)).await;
            }
        }

        // Fetch via session_manager
        let fetch_started = tokio::time::Instant::now();
        let result = session_manager.fetch(&request).await;

        match result {
            Ok(response) => {
                let status = response.status();
                let content_len = response.content_length() as u64;

                if let Some(ref throttle) = throttle {
                    let latency = fetch_started.elapsed().as_secs_f64();
                    let retry_after = response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| parse_retry_after(v));
                    let mut t = throttle.lock().unwrap_or_else(|p| p.into_inner());
                    t.record(
                        &domain,
                        latency,
                        status,
                        retry_after,
                        delay_floor,
                        tokio::time::Instant::now(),
                    );
                }

                // Update stats
                {
                    let mut s = stats.lock().await;
                    s.increment_requests_count();
                    s.increment_status(status);
                    s.increment_response_bytes(content_len);
                    if !domain.is_empty() {
                        *s.domains_response_bytes.entry(domain.clone()).or_insert(0) += content_len;
                    }
                    let session = if request.session_id().is_empty() {
                        "default"
                    } else {
                        request.session_id()
                    };
                    *s.sessions_requests_count
                        .entry(session.to_string())
                        .or_insert(0) += 1;
                }

                let spider_resp = SpiderResponse::new(response);

                // Check if blocked — BEFORE the dev-cache write. Caching a
                // blocked response would serve the same blocked page back to
                // the re-enqueued retry from disk, defeating the retry loop
                // entirely (and permanently, until the cache is cleared).
                if spider.is_blocked(&spider_resp).await {
                    let mut s = stats.lock().await;
                    s.blocked_requests_count += 1;
                    drop(s);

                    if request.retry_count() < spider.max_blocked_retries() {
                        let mut retry_req = request.copy();
                        retry_req.increment_retry();
                        retry_req.set_dont_filter(true);
                        scheduler.lock().await.enqueue(retry_req);
                    }
                    return;
                }

                // Store to cache if dev mode (only non-blocked responses).
                if let Some(ref response_cache) = cache {
                    let response = spider_resp.response();
                    let cached = CachedResponse {
                        status: response.status(),
                        content_type: response.content_type().to_string(),
                        body: response.text().to_string(),
                        url: response.url().to_string(),
                        headers: response.headers().clone(),
                    };
                    let _ = response_cache.put(&url, &cached).await;
                }

                // Parse response
                let (parsed_items, follow_requests) = spider.parse(spider_resp).await;
                Self::collect_items(&spider, &items, &stats, parsed_items).await;
                Self::enqueue_follow_requests(&scheduler, follow_requests).await;
            }
            Err(err) => {
                // Surface the typed SessionError as a string to the spider hook
                // (NotFound vs Network vs UnsupportedMethod is preserved in the
                // message; the typed error is available to direct fetch callers).
                spider.on_error(&request, &err.to_string()).await;
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
