//! `CrawlRule` and `CrawlSpider`, ported from upstream Scrapling's
//! `spiders/templates/crawler.py` (v0.4.8).
//!
//! The Python original dispatches each matched URL to a per-rule callback;
//! this port has no per-request callbacks (the [`Spider`] trait routes every
//! response through `parse`), so a rule instead carries an optional
//! `process_request` hook and priority override, and item extraction is a
//! spider-level closure.

use crate::spiders::links::LinkExtractor;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

/// Hook to mutate (or drop, by returning `None`) each request built from a
/// rule before it is dispatched.
pub type ProcessRequestFn =
    Arc<dyn Fn(SpiderRequest, &SpiderResponse) -> Option<SpiderRequest> + Send + Sync>;

/// Item-extraction hook for template spiders: returns the items scraped from
/// a content page.
pub type ParseItemFn = Arc<dyn Fn(&SpiderResponse) -> Vec<serde_json::Value> + Send + Sync>;

/// Rule for [`CrawlSpider`]: extract links from a response and dispatch them.
#[derive(Clone)]
#[must_use = "a CrawlRule does nothing until added to a spider via `.rule()`/`.rules()`"]
pub struct CrawlRule {
    /// Selects and filters which links this rule follows.
    pub link_extractor: LinkExtractor,
    /// Override the priority of the requests that will be dispatched.
    pub priority: Option<i32>,
    /// Optional hook to mutate each request before it is dispatched; return
    /// `None` to drop the request.
    pub process_request: Option<ProcessRequestFn>,
}

impl std::fmt::Debug for CrawlRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrawlRule")
            .field("link_extractor", &self.link_extractor)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

impl CrawlRule {
    /// Create a rule following the links selected by `link_extractor`,
    /// with no priority override and no request hook.
    pub fn new(link_extractor: LinkExtractor) -> Self {
        Self {
            link_extractor,
            priority: None,
            process_request: None,
        }
    }

    /// Dispatch this rule's requests with the given scheduling priority
    /// (higher is crawled first).
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set a hook that can mutate each built request (add headers, meta,
    /// a session) or drop it by returning `None`.
    pub fn process_request(mut self, f: ProcessRequestFn) -> Self {
        self.process_request = Some(f);
        self
    }

    /// Build a request for `url` according to this rule.
    pub fn build_request(&self, url: &str, response: &SpiderResponse) -> Option<SpiderRequest> {
        let mut builder = SpiderRequest::builder(url);
        if let Some(p) = self.priority {
            builder = builder.priority(p);
        }
        let req = builder.build();
        match &self.process_request {
            Some(f) => f(req, response),
            None => Some(req),
        }
    }

    /// Extract links from `response` with this rule's extractor and build a
    /// request for each.
    pub fn requests(&self, response: &SpiderResponse) -> Vec<SpiderRequest> {
        self.link_extractor
            .extract(response)
            .iter()
            .filter_map(|url| self.build_request(url, response))
            .collect()
    }
}

/// Apply every rule to `response` and collect the follow-up requests.
pub fn follow_rules(rules: &[CrawlRule], response: &SpiderResponse) -> Vec<SpiderRequest> {
    rules.iter().flat_map(|r| r.requests(response)).collect()
}

/// A generic spider that extracts and follows links automatically based on
/// crawl rules.
///
/// Configure it with [`CrawlSpider::builder`]: rules define which links are
/// followed, and an optional `parse_item` closure extracts items from every
/// fetched page.
pub struct CrawlSpider {
    name: String,
    start_urls: Vec<String>,
    rules: Vec<CrawlRule>,
    parse_item: Option<ParseItemFn>,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    development_mode: bool,
    robots_txt_obey: bool,
}

impl CrawlSpider {
    /// Start building a `CrawlSpider` with the given spider name.
    pub fn builder(name: &str) -> CrawlSpiderBuilder {
        CrawlSpiderBuilder::new(name)
    }

    /// The configured crawl rules.
    pub fn rules(&self) -> &[CrawlRule] {
        &self.rules
    }
}

#[async_trait]
impl Spider for CrawlSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    fn allowed_domains(&self) -> HashSet<String> {
        self.allowed_domains.clone()
    }

    fn concurrent_requests(&self) -> u32 {
        self.concurrent_requests
    }

    fn development_mode(&self) -> bool {
        self.development_mode
    }

    fn robots_txt_obey(&self) -> bool {
        self.robots_txt_obey
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let items = self
            .parse_item
            .as_ref()
            .map(|f| f(&response))
            .unwrap_or_default();
        let requests = follow_rules(&self.rules, &response);
        (items, requests)
    }
}

/// Builder for [`CrawlSpider`], created via [`CrawlSpider::builder`].
#[must_use = "builders do nothing until `.build()` is called"]
pub struct CrawlSpiderBuilder {
    spider: CrawlSpider,
}

impl CrawlSpiderBuilder {
    fn new(name: &str) -> Self {
        Self {
            spider: CrawlSpider {
                name: name.to_string(),
                start_urls: Vec::new(),
                rules: Vec::new(),
                parse_item: None,
                allowed_domains: HashSet::new(),
                concurrent_requests: 4,
                development_mode: false,
                robots_txt_obey: false,
            },
        }
    }

    /// Add seed URLs the crawl starts from.
    pub fn start_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.spider
            .start_urls
            .extend(urls.into_iter().map(|u| u.as_ref().to_string()));
        self
    }

    /// Add one crawl rule (all rules run against every fetched page).
    pub fn rule(mut self, rule: CrawlRule) -> Self {
        self.spider.rules.push(rule);
        self
    }

    /// Add several crawl rules at once.
    pub fn rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = CrawlRule>,
    {
        self.spider.rules.extend(rules);
        self
    }

    /// Closure that extracts items from each fetched page.
    pub fn parse_item(mut self, f: ParseItemFn) -> Self {
        self.spider.parse_item = Some(f);
        self
    }

    /// Restrict the crawl to these hosts (see
    /// [`Spider::allowed_domains`]).
    pub fn allowed_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.spider
            .allowed_domains
            .extend(domains.into_iter().map(|d| d.as_ref().to_string()));
        self
    }

    /// Set the global concurrency limit (default 4).
    pub fn concurrent_requests(mut self, n: u32) -> Self {
        self.spider.concurrent_requests = n;
        self
    }

    /// Enable development mode (cache responses on disk and replay them —
    /// see [`Spider::development_mode`]).
    pub fn development_mode(mut self, on: bool) -> Self {
        self.spider.development_mode = on;
        self
    }

    /// Fetch and obey each origin's robots.txt (see
    /// [`Spider::robots_txt_obey`]).
    pub fn robots_txt_obey(mut self, on: bool) -> Self {
        self.spider.robots_txt_obey = on;
        self
    }

    /// Finish the builder and return the configured [`CrawlSpider`].
    #[must_use]
    pub fn build(self) -> CrawlSpider {
        self.spider
    }
}
