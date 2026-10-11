//! `SiteToMarkdownSpider`, ported from upstream Scrapling's
//! `SiteToMarkdownSpider` (v0.4.15): crawl a website and convert every page
//! to clean, LLM-ready Markdown for RAG/ingestion pipelines.

use crate::spiders::links::LinkExtractor;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use crate::spiders::templates::crawler::{follow_rules, CrawlRule};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A [`Spider`] that crawls a website and converts every page to Markdown.
///
/// Yields one item per page with `url`, `title`, and `markdown` keys, and
/// when `output_dir` is set, also writes each page to a Markdown file named
/// after its URL. Every page link inside `allowed_domains` is followed by
/// default; set `rules` with your own [`LinkExtractor`] to narrow the crawl
/// or drop URL patterns.
///
/// `allowed_domains` is required so the crawl stays bound to the target
/// website(s). The conversion always strips scripts, styles, and hidden /
/// prompt-injection content; see the
/// [`markdown`](crate::parser::markdown) module docs.
pub struct SiteToMarkdownSpider {
    name: String,
    start_urls: Vec<String>,
    rules: Vec<CrawlRule>,
    css_selector: Option<String>,
    main_content_only: bool,
    output_dir: Option<PathBuf>,
    max_pages: usize,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    development_mode: bool,
    robots_txt_obey: bool,
    page_count: AtomicUsize,
    used_names: Mutex<HashSet<String>>,
}

impl SiteToMarkdownSpider {
    /// Start building a `SiteToMarkdownSpider` with the given spider name.
    pub fn builder(name: &str) -> SiteToMarkdownSpiderBuilder {
        SiteToMarkdownSpiderBuilder::new(name)
    }

    /// Build a unique filesystem-safe name from the URL, suffixing a hash
    /// of the URL on collisions (upstream uses SHA-1; this port uses the
    /// SHA-256 already in the tree).
    fn filename_for(&self, url: &str) -> String {
        let base = match url::Url::parse(url) {
            Ok(u) => format!("{}{}", u.host_str().unwrap_or_default(), u.path()),
            Err(_) => url.to_string(),
        };
        let mut name = String::new();
        let mut last_dash = true; // suppress leading dashes
        for c in base.trim_matches('/').chars() {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                name.push(c);
                last_dash = false;
            } else if !last_dash {
                name.push('-');
                last_dash = true;
            }
        }
        let mut name = name.trim_matches('-').to_string();
        name.truncate(150);
        if name.is_empty() {
            name = "index".to_string();
        }
        let mut used = self.used_names.lock().expect("filename lock poisoned");
        if used.contains(&name) {
            use sha2::{Digest, Sha256};
            let digest = crate::core::hash::hex(&Sha256::digest(url.as_bytes()));
            name = format!("{name}-{}", &digest[..8]);
        }
        used.insert(name.clone());
        name
    }
}

#[async_trait]
impl Spider for SiteToMarkdownSpider {
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
        // Reserve a conversion slot; pages beyond the cap are neither
        // converted nor followed. Requests already queued when the cap hits
        // may still be fetched, matching upstream.
        if self.max_pages > 0 {
            // Compare-exchange loop instead of `fetch_update`: the latter
            // was deprecated (renamed) on newer toolchains while the new
            // name is not available on the MSRV.
            let mut count = self.page_count.load(Ordering::SeqCst);
            loop {
                if count >= self.max_pages {
                    return (vec![], vec![]);
                }
                match self.page_count.compare_exchange(
                    count,
                    count + 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(actual) => count = actual,
                }
            }
        } else {
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }

        let title = response
            .css_get("title::text")
            .map(|t| t.as_str().trim().to_string())
            .unwrap_or_default();
        let markdown = response.markdown(self.css_selector.as_deref(), self.main_content_only);

        if let Some(dir) = &self.output_dir {
            let file = dir.join(format!("{}.md", self.filename_for(response.url())));
            let write = async {
                tokio::fs::create_dir_all(dir).await?;
                tokio::fs::write(&file, &markdown).await
            };
            if let Err(e) = write.await {
                log::warn!("Failed to write Markdown file {}: {e}", file.display());
            }
        }

        let item = serde_json::json!({
            "url": response.url(),
            "title": title,
            "markdown": markdown,
        });

        let under_cap =
            self.max_pages == 0 || self.page_count.load(Ordering::SeqCst) < self.max_pages;
        let requests = if under_cap {
            follow_rules(&self.rules, &response)
        } else {
            vec![]
        };
        (vec![item], requests)
    }
}

/// Builder for [`SiteToMarkdownSpider`].
#[must_use = "a builder does nothing until `.build()` is called"]
pub struct SiteToMarkdownSpiderBuilder {
    spider: SiteToMarkdownSpider,
    rules_set: bool,
}

impl SiteToMarkdownSpiderBuilder {
    fn new(name: &str) -> Self {
        Self {
            spider: SiteToMarkdownSpider {
                name: name.to_string(),
                start_urls: Vec::new(),
                rules: vec![CrawlRule::new(LinkExtractor::new())],
                css_selector: None,
                main_content_only: true,
                output_dir: None,
                max_pages: 0,
                allowed_domains: HashSet::new(),
                concurrent_requests: 4,
                development_mode: false,
                robots_txt_obey: false,
                page_count: AtomicUsize::new(0),
                used_names: Mutex::new(HashSet::new()),
            },
            rules_set: false,
        }
    }

    /// Add a seed URL the crawl starts from.
    pub fn start_url(mut self, url: &str) -> Self {
        self.spider.start_urls.push(url.to_string());
        self
    }

    /// Add several seed URLs.
    pub fn start_urls(mut self, urls: impl IntoIterator<Item = String>) -> Self {
        self.spider.start_urls.extend(urls);
        self
    }

    /// Restrict the crawl to these domains. **Required**: `build()` panics
    /// when no domain is set, so the crawl stays bound to the target
    /// website(s).
    pub fn allowed_domains(mut self, domains: impl IntoIterator<Item = String>) -> Self {
        self.spider.allowed_domains.extend(domains);
        self
    }

    /// Replace the default follow-every-link rule with custom crawl rules
    /// to narrow the crawl or drop URL patterns.
    pub fn rules(mut self, rules: impl IntoIterator<Item = CrawlRule>) -> Self {
        if !self.rules_set {
            self.spider.rules.clear();
            self.rules_set = true;
        }
        self.spider.rules.extend(rules);
        self
    }

    /// Convert only the elements matching this CSS selector on each page
    /// (all matches are concatenated).
    pub fn css_selector(mut self, selector: &str) -> Self {
        self.spider.css_selector = Some(selector.to_string());
        self
    }

    /// Convert only the content inside each page's `<body>` tag (default
    /// `true`).
    pub fn main_content_only(mut self, on: bool) -> Self {
        self.spider.main_content_only = on;
        self
    }

    /// When set, each page is also written to this directory as a Markdown
    /// file named after its URL.
    pub fn output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.spider.output_dir = Some(dir.into());
        self
    }

    /// Maximum number of pages to convert; `0` disables the cap (default).
    /// Requests already queued when the cap hits may still be fetched, but
    /// they are neither converted nor followed.
    pub fn max_pages(mut self, n: usize) -> Self {
        self.spider.max_pages = n;
        self
    }

    /// Global concurrency limit (default 4).
    pub fn concurrent_requests(mut self, n: u32) -> Self {
        self.spider.concurrent_requests = n;
        self
    }

    /// Cache responses to disk for development iteration.
    pub fn development_mode(mut self, on: bool) -> Self {
        self.spider.development_mode = on;
        self
    }

    /// Respect robots.txt.
    pub fn robots_txt_obey(mut self, on: bool) -> Self {
        self.spider.robots_txt_obey = on;
        self
    }

    /// Finish building the spider.
    ///
    /// # Panics
    ///
    /// Panics when no `allowed_domains` were set — the crawl must stay
    /// bound to the target website(s), matching upstream's `ValueError`.
    pub fn build(self) -> SiteToMarkdownSpider {
        assert!(
            !self.spider.allowed_domains.is_empty(),
            "SiteToMarkdownSpider requires `allowed_domains` to bound the crawl to the target website(s)"
        );
        self.spider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spider(max_pages: usize) -> SiteToMarkdownSpider {
        SiteToMarkdownSpider::builder("test")
            .start_url("https://example.com/")
            .allowed_domains(["example.com".to_string()])
            .max_pages(max_pages)
            .build()
    }

    #[test]
    fn filenames_are_sanitized_unique_and_bounded() {
        let s = spider(0);
        assert_eq!(
            s.filename_for("https://example.com/docs/intro?x=1"),
            "example.com-docs-intro"
        );
        // Same URL path again: collision gets a hash suffix.
        let second = s.filename_for("https://example.com/docs/intro/");
        assert!(second.starts_with("example.com-docs-intro-"));
        assert_eq!(second.len(), "example.com-docs-intro".len() + 9);
        // Bare origin falls back to the host.
        assert_eq!(s.filename_for("https://example.com/"), "example.com");
        let long = format!("https://example.com/{}", "a".repeat(400));
        assert!(s.filename_for(&long).len() <= 150);
    }

    #[test]
    #[should_panic(expected = "allowed_domains")]
    fn build_without_allowed_domains_panics() {
        let _ = SiteToMarkdownSpider::builder("bad")
            .start_url("https://example.com/")
            .build();
    }
}
