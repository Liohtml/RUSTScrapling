//! The [`Spider`] trait: implement it to define what a crawl fetches and
//! how responses are parsed, then run it with
//! [`CrawlerEngine`](crate::spiders::engine::CrawlerEngine).

use crate::fetchers::config::FetcherConfig;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use async_trait::async_trait;
use std::collections::HashSet;

/// The core Spider trait users implement to define a crawler.
///
/// Only [`Spider::name`], [`Spider::start_urls`], and [`Spider::parse`]
/// are required; everything else is engine configuration and lifecycle
/// hooks with sensible defaults. Implement it with the
/// [`async_trait`](https://docs.rs/async-trait) attribute macro.
#[async_trait]
pub trait Spider: Send + Sync + 'static {
    /// Unique spider name. Also used (sanitized) as the default directory
    /// name for the dev cache and checkpoints (`.scrapling/<name>/…`).
    fn name(&self) -> &str;

    /// The URLs the crawl is seeded with. The default
    /// [`Spider::start_requests`] turns each into a plain GET request.
    fn start_urls(&self) -> Vec<String>;

    // Config with defaults

    /// Hosts the crawl is confined to (exact match on the request URL's
    /// host). Empty — the default — means no restriction. Requests to other
    /// hosts, and requests whose URL has no parseable host, are dropped and
    /// counted in `offsite_requests_count`.
    fn allowed_domains(&self) -> HashSet<String> {
        HashSet::new()
    }
    /// Whether the engine fetches and obeys each origin's robots.txt
    /// (RFC 9309 rules plus `Crawl-delay`). Off by default.
    fn robots_txt_obey(&self) -> bool {
        false
    }
    /// Maximum number of requests in flight at once across all domains
    /// (values below 1 are treated as 1). Default: 4.
    fn concurrent_requests(&self) -> u32 {
        4
    }
    /// Maximum number of requests in flight at once *per domain*.
    /// `0` — the default — means no per-domain limit.
    fn concurrent_requests_per_domain(&self) -> u32 {
        0
    }
    /// Fixed politeness delay in seconds between request dispatches
    /// (skipped for requests served from the dev cache). With AutoThrottle
    /// enabled this value instead becomes the floor of the adaptive
    /// per-domain delay. Default: 0.
    fn download_delay(&self) -> f64 {
        0.0
    }
    /// Enable AutoThrottle: adaptive per-domain delays based on measured
    /// response latency, doubling on 429/503 (honoring `Retry-After`) and
    /// easing back down when the server is healthy. `download_delay` and the
    /// site's robots.txt `Crawl-delay` act as floors that are never
    /// undercut. When enabled, the static `download_delay` pacing is
    /// replaced by the adaptive per-domain schedule.
    fn autothrottle_enabled(&self) -> bool {
        false
    }
    /// Initial per-domain delay (seconds) until latency data exists.
    fn autothrottle_start_delay(&self) -> f64 {
        5.0
    }
    /// Hard upper bound (seconds) the adaptive delay never exceeds.
    fn autothrottle_max_delay(&self) -> f64 {
        60.0
    }
    /// Desired average number of in-flight requests per domain; the adaptive
    /// delay targets `latency / target_concurrency`.
    fn autothrottle_target_concurrency(&self) -> f64 {
        1.0
    }
    /// How many times a request classified as blocked (see
    /// [`Spider::is_blocked`]) is re-enqueued before being given up on.
    /// Default: 3.
    fn max_blocked_retries(&self) -> u32 {
        3
    }
    /// Whether request `meta` entries participate in the dedup
    /// fingerprint, so otherwise-identical requests with different meta
    /// count as distinct. Default: off.
    fn fp_include_kwargs(&self) -> bool {
        false
    }
    /// Whether URL fragments (`#…`) participate in the dedup fingerprint.
    /// Off by default, so `page#a` and `page#b` dedupe to one request.
    fn fp_keep_fragments(&self) -> bool {
        false
    }
    /// Whether request headers participate in the dedup fingerprint.
    /// Default: off.
    fn fp_include_headers(&self) -> bool {
        false
    }
    /// Development mode: every non-blocked response is cached on disk and
    /// replayed on later runs, so spider code can be iterated on without
    /// re-hitting the site. Off by default.
    fn development_mode(&self) -> bool {
        false
    }

    /// The initial requests of the crawl. Defaults to a plain GET per
    /// [`Spider::start_urls`] entry; override to customize method, headers,
    /// priority, etc.
    fn start_requests(&self) -> Vec<SpiderRequest> {
        self.start_urls()
            .into_iter()
            .map(|url| SpiderRequest::new(&url))
            .collect()
    }

    /// Main callback, invoked for every successfully fetched (and
    /// non-blocked) response. Returns the scraped items plus any follow-up
    /// requests to enqueue (duplicates are filtered by the scheduler).
    async fn parse(&self, response: SpiderResponse)
        -> (Vec<serde_json::Value>, Vec<SpiderRequest>);

    // Hooks with default no-op implementations

    /// Called once before crawling begins. `resuming` is `true` when the
    /// crawl was restored from a checkpoint instead of starting fresh.
    async fn on_start(&self, _resuming: bool) {}
    /// Called once after the crawl loop ends (both on completion and on
    /// pause).
    async fn on_close(&self) {}
    /// Called when a request fails at the network/session level (it does
    /// NOT fire for blocked responses — those go through the retry loop).
    async fn on_error(&self, _request: &SpiderRequest, _error: &str) {}
    /// Item pipeline hook: called for every item returned by
    /// [`Spider::parse`]. Return the (possibly transformed) item to keep
    /// it, or `None` to drop it (counted in `items_dropped`).
    async fn on_scraped_item(&self, item: serde_json::Value) -> Option<serde_json::Value> {
        Some(item)
    }
    /// Decide whether a response means the crawler was blocked; blocked
    /// requests are re-enqueued up to [`Spider::max_blocked_retries`]
    /// times and never cached. Defaults to the status-code check
    /// [`SpiderResponse::is_blocked`]; override to detect soft blocks
    /// (CAPTCHAs, "are you human" pages) by inspecting the body.
    async fn is_blocked(&self, response: &SpiderResponse) -> bool {
        response.is_blocked()
    }

    /// HTTP configuration for the crawl's default session (timeouts,
    /// retries, proxies, stealth headers, …).
    fn fetcher_config(&self) -> FetcherConfig {
        FetcherConfig::default()
    }
}
