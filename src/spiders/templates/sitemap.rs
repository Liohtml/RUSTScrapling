//! `SitemapSpider`, ported from upstream Scrapling's
//! `spiders/templates/sitemap.py` (v0.4.8).
//!
//! The Python original routes sitemap responses through a dedicated
//! `_parse_sitemap` callback; this port has no per-request callbacks, so
//! sitemap documents are recognized by inspecting the response itself
//! (robots.txt path, `<sitemapindex>`/`<urlset>` root elements) and every
//! other response is treated as a content page.
//!
//! Gzipped sitemaps: responses compressed at the transport level
//! (`Content-Encoding: gzip`) are decompressed transparently by the HTTP
//! client. Raw `.xml.gz` *files* are not supported because response bodies
//! are stored as text.

use crate::parser::Selector;
use crate::spiders::links::LinkExtractor;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use crate::spiders::templates::crawler::{CrawlRule, ParseItemFn};
use async_trait::async_trait;
use std::collections::HashSet;
use url::Url;

/// Parsed sitemap body: `urls` holds the entries from a `<urlset>`;
/// `sitemaps` holds child sitemap URLs from a `<sitemapindex>` (each of
/// which is fetched recursively).
#[derive(Debug, Default, PartialEq)]
pub struct SitemapResult {
    pub urls: Vec<String>,
    pub sitemaps: Vec<String>,
}

/// A [`Spider`] that seeds a crawl from sitemap(s) and follows the rules.
///
/// Sitemap URLs (or robots.txt URLs, whose `Sitemap:` directives are
/// followed) go in `sitemap_urls`. Page URLs found in sitemaps are
/// dispatched through the rules — first matching rule wins, unmatched URLs
/// are dropped unless no rules are configured (then everything is
/// followed). Content pages are handed to the `parse_item` closure.
pub struct SitemapSpider {
    name: String,
    sitemap_urls: Vec<String>,
    rules: Vec<CrawlRule>,
    /// Filter for which child sitemaps inside a `<sitemapindex>` to descend
    /// into. `None` means descend into all.
    sitemap_follow: Option<LinkExtractor>,
    /// When enabled, alternate-language URLs (`<xhtml:link href=..>`) are
    /// also routed through the rules.
    sitemap_alternate_links: bool,
    parse_item: Option<ParseItemFn>,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    development_mode: bool,
    robots_txt_obey: bool,
}

impl SitemapSpider {
    pub fn builder(name: &str) -> SitemapSpiderBuilder {
        SitemapSpiderBuilder::new(name)
    }

    /// Extract `Sitemap:` directives from a robots.txt body.
    fn robots_sitemaps(body: &str) -> Vec<String> {
        body.lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line
                    .get(..8)
                    .filter(|p| p.eq_ignore_ascii_case("sitemap:"))
                    .map(|_| line[8..].trim())?;
                (!rest.is_empty()).then(|| rest.to_string())
            })
            .collect()
    }

    /// Parse a sitemap body and return its URLs and any child sitemaps.
    pub fn parse_sitemap_body(&self, body: &str) -> SitemapResult {
        let root = Selector::from_html(body);

        let index_locs: Vec<String> = root
            .css("sitemapindex sitemap > loc")
            .into_iter()
            .map(|el| el.text().to_string().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !index_locs.is_empty() {
            return SitemapResult {
                urls: Vec::new(),
                sitemaps: index_locs,
            };
        }

        let mut urls: Vec<String> = Vec::new();
        for url_el in root.css("urlset url") {
            for child in url_el.children() {
                let tag = child.tag().to_string();
                if tag == "loc" {
                    let text = child.text().to_string().trim().to_string();
                    if !text.is_empty() {
                        urls.push(text);
                    }
                } else if self.sitemap_alternate_links && (tag == "link" || tag.ends_with(":link"))
                {
                    if let Some(href) = child.get_attribute("href") {
                        let href = href.to_string().trim().to_string();
                        if !href.is_empty() {
                            urls.push(href);
                        }
                    }
                }
            }
        }
        if urls.is_empty() && root.css("urlset").into_iter().next().is_none() {
            log::warn!("Response does not look like a sitemap (no <urlset> or <sitemapindex>)");
        }
        SitemapResult {
            urls,
            sitemaps: Vec::new(),
        }
    }

    /// Dispatch a page URL through the rules: first match wins; unmatched
    /// URLs are dropped unless no rules are configured.
    fn dispatch(&self, url: &str, response: &SpiderResponse) -> Option<SpiderRequest> {
        if self.rules.is_empty() {
            return Some(SpiderRequest::new(url));
        }
        self.rules
            .iter()
            .find(|rule| rule.link_extractor.matches(url))
            .and_then(|rule| rule.build_request(url, response))
    }

    fn is_robots_txt(url: &str) -> bool {
        Url::parse(url)
            .map(|u| u.path().ends_with("/robots.txt"))
            .unwrap_or(false)
    }

    fn looks_like_sitemap(body: &str) -> bool {
        let head: String = body.chars().take(4096).collect::<String>().to_lowercase();
        head.contains("<urlset") || head.contains("<sitemapindex")
    }
}

#[async_trait]
impl Spider for SitemapSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.sitemap_urls.clone()
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
        // robots.txt: follow its Sitemap directives.
        if Self::is_robots_txt(response.url()) {
            let sitemaps = Self::robots_sitemaps(response.text());
            if sitemaps.is_empty() {
                log::warn!("No Sitemaps found in {}", response.url());
            }
            let requests = sitemaps.iter().map(|u| SpiderRequest::new(u)).collect();
            return (Vec::new(), requests);
        }

        // Sitemap document: descend into child sitemaps and dispatch URLs.
        if Self::looks_like_sitemap(response.text()) {
            let result = self.parse_sitemap_body(response.text());
            let mut requests: Vec<SpiderRequest> = Vec::new();
            for child in &result.sitemaps {
                if let Some(filter) = &self.sitemap_follow {
                    if !filter.matches(child) {
                        continue;
                    }
                }
                requests.push(SpiderRequest::new(child));
            }
            for url in &result.urls {
                if let Some(req) = self.dispatch(url, &response) {
                    requests.push(req);
                }
            }
            return (Vec::new(), requests);
        }

        // Content page.
        let items = self
            .parse_item
            .as_ref()
            .map(|f| f(&response))
            .unwrap_or_default();
        (items, Vec::new())
    }
}

pub struct SitemapSpiderBuilder {
    spider: SitemapSpider,
}

impl SitemapSpiderBuilder {
    fn new(name: &str) -> Self {
        Self {
            spider: SitemapSpider {
                name: name.to_string(),
                sitemap_urls: Vec::new(),
                rules: Vec::new(),
                sitemap_follow: None,
                sitemap_alternate_links: false,
                parse_item: None,
                allowed_domains: HashSet::new(),
                concurrent_requests: 4,
                development_mode: false,
                robots_txt_obey: false,
            },
        }
    }

    /// Explicit list of sitemap (or robots.txt) URLs to fetch.
    pub fn sitemap_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.spider
            .sitemap_urls
            .extend(urls.into_iter().map(|u| u.as_ref().to_string()));
        self
    }

    pub fn rule(mut self, rule: CrawlRule) -> Self {
        self.spider.rules.push(rule);
        self
    }

    pub fn rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = CrawlRule>,
    {
        self.spider.rules.extend(rules);
        self
    }

    /// Filter which child sitemaps inside a `<sitemapindex>` to descend into.
    pub fn sitemap_follow(mut self, extractor: LinkExtractor) -> Self {
        self.spider.sitemap_follow = Some(extractor);
        self
    }

    /// Also route alternate-language links through the rules.
    pub fn sitemap_alternate_links(mut self, on: bool) -> Self {
        self.spider.sitemap_alternate_links = on;
        self
    }

    /// Closure that extracts items from each content page.
    pub fn parse_item(mut self, f: ParseItemFn) -> Self {
        self.spider.parse_item = Some(f);
        self
    }

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

    pub fn concurrent_requests(mut self, n: u32) -> Self {
        self.spider.concurrent_requests = n;
        self
    }

    pub fn development_mode(mut self, on: bool) -> Self {
        self.spider.development_mode = on;
        self
    }

    pub fn robots_txt_obey(mut self, on: bool) -> Self {
        self.spider.robots_txt_obey = on;
        self
    }

    pub fn build(self) -> SitemapSpider {
        self.spider
    }
}
