use crate::fetchers::config::FetcherConfig;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use async_trait::async_trait;
use std::collections::HashSet;

/// The core Spider trait users implement to define a crawler.
#[async_trait]
pub trait Spider: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn start_urls(&self) -> Vec<String>;

    // Config with defaults
    fn allowed_domains(&self) -> HashSet<String> { HashSet::new() }
    fn robots_txt_obey(&self) -> bool { false }
    fn concurrent_requests(&self) -> u32 { 4 }
    fn concurrent_requests_per_domain(&self) -> u32 { 0 }
    fn download_delay(&self) -> f64 { 0.0 }
    fn max_blocked_retries(&self) -> u32 { 3 }
    fn fp_include_kwargs(&self) -> bool { false }
    fn fp_keep_fragments(&self) -> bool { false }
    fn fp_include_headers(&self) -> bool { false }
    fn development_mode(&self) -> bool { false }

    fn start_requests(&self) -> Vec<SpiderRequest> {
        self.start_urls().into_iter().map(|url| SpiderRequest::new(&url)).collect()
    }

    /// Main callback - returns (items, follow_requests)
    async fn parse(&self, response: SpiderResponse) -> (Vec<serde_json::Value>, Vec<SpiderRequest>);

    // Hooks with default no-op implementations
    async fn on_start(&self, _resuming: bool) {}
    async fn on_close(&self) {}
    async fn on_error(&self, _request: &SpiderRequest, _error: &str) {}
    async fn on_scraped_item(&self, item: serde_json::Value) -> Option<serde_json::Value> { Some(item) }
    async fn is_blocked(&self, response: &SpiderResponse) -> bool { response.is_blocked() }

    fn fetcher_config(&self) -> FetcherConfig { FetcherConfig::default() }
}
