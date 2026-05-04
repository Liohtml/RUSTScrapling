use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;
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

pub struct CrawlerEngine<S: Spider> {
    spider: Arc<S>,
    session_manager: Arc<SessionManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    stats: Arc<Mutex<CrawlStats>>,
    items: Arc<Mutex<ItemList>>,
    global_limiter: Arc<Semaphore>,
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
                .unwrap_or_else(|| format!(".scrapling/{}/cache", spider.name()));
            ResponseCache::new(&dir).ok().map(Arc::new)
        } else {
            None
        };

        let checkpoint = {
            let dir = crawl_dir
                .map(|d| format!("{}/checkpoints", d))
                .unwrap_or_else(|| format!(".scrapling/{}/checkpoints", spider.name()));
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
                if let Some(data) = cp.restore() {
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
                if let Some(domain) = Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_string())) {
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
                    let permit = self.global_limiter.clone().acquire_owned().await;
                    if permit.is_err() {
                        break;
                    }
                    let permit = permit.unwrap();

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

                        active_tasks.fetch_sub(1, Ordering::SeqCst);
                        drop(permit);
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

        // On pause, save checkpoint
        let was_paused = self.paused.load(Ordering::SeqCst);
        if was_paused {
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
                let _ = cp.save(&data);
            }
        } else {
            // Clean up checkpoint on successful completion
            if let Some(ref cp) = self.checkpoint {
                cp.cleanup();
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

        // Check robots.txt
        if let Some(ref robots) = robots_manager {
            let robots = robots.lock().await;
            if !robots.is_allowed(&url) {
                stats.lock().await.robots_disallowed_count += 1;
                return;
            }
        }

        // Check allowed_domains
        let allowed = spider.allowed_domains();
        if !allowed.is_empty() {
            if let Some(domain) = request.domain() {
                if !allowed.contains(&domain) {
                    stats.lock().await.offsite_requests_count += 1;
                    return;
                }
            }
        }

        // Check dev cache
        if let Some(ref response_cache) = cache {
            if let Some(cached) = response_cache.get(&url) {
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

        // Apply download delay
        let delay = spider.download_delay();
        if delay > 0.0 {
            tokio::time::sleep(std::time::Duration::from_secs_f64(delay)).await;
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
                    let _ = response_cache.put(&url, &cached);
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
