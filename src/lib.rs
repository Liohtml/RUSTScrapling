//! RUSTScrapling — a Rust port of the [Scrapling](https://github.com/D4Vinci/Scrapling)
//! web-scraping framework.
//!
//! The crate is organized in three layers, each usable on its own:
//!
//! - **[`parser`]** — HTML parsing and data extraction. [`Selector`] wraps a
//!   parsed DOM node and offers CSS selection (including Parsel-style
//!   `::text` / `::attr(name)` pseudo-elements via [`Selector::css_get`] /
//!   [`Selector::css_getall`]), text extraction, DOM navigation, regex
//!   helpers, JSON parsing, and *adaptive element relocation* — a saved
//!   element's "unique properties" can be used to find it again by
//!   similarity after the page layout changes.
//! - **[`fetchers`]** — async HTTP. [`Fetcher`] is a reqwest-backed client
//!   configured through [`FetcherConfig`] (timeouts, retries, redirects,
//!   single/per-protocol/rotating proxies, response-size caps) that sends
//!   browser-like *stealth headers* by default, and returns a [`Response`]
//!   with charset-aware body decoding and direct parser integration.
//! - **[`spiders`]** — crawling. Implement the [`Spider`] trait (or use a
//!   ready-made template: [`CrawlSpider`], [`SitemapSpider`],
//!   [`ShopifySpider`]) and run it with [`CrawlerEngine`]. The engine
//!   handles concurrency limits (global and per-domain), request
//!   deduplication, robots.txt compliance (RFC 9309 matching plus
//!   `Crawl-delay`), AutoThrottle adaptive per-domain delays,
//!   blocked-response retries, checkpoints for pause/resume, and a
//!   development-mode response cache for offline iteration. Results come
//!   back as a [`CrawlResult`] with live stats and an [`ItemList`]
//!   exportable to CSV, JSON, JSON Lines, or XML.
//!
//! # Quickstart: parsing HTML
//!
//! ```
//! use rust_scrapling::Selector;
//!
//! let html = r#"<html><body>
//!   <ul>
//!     <li class="item"><a href="/p/1">Widget</a> <span class="price">9.99</span></li>
//!     <li class="item"><a href="/p/2">Gadget</a> <span class="price">19.99</span></li>
//!   </ul>
//! </body></html>"#;
//!
//! let page = Selector::from_html(html);
//!
//! // Parsel-style `::text` / `::attr()` pseudo-elements:
//! let names: Vec<String> = page
//!     .css_getall("li.item a::text")
//!     .into_iter()
//!     .map(|t| t.into_string())
//!     .collect();
//! assert_eq!(names, ["Widget", "Gadget"]);
//!
//! let first_href = page.css_get("li.item a::attr(href)").unwrap();
//! assert_eq!(first_href.as_str(), "/p/1");
//!
//! // Plain CSS queries return `Selectors` for further navigation:
//! let prices = page.css(".price");
//! assert_eq!(prices.len(), 2);
//! assert_eq!(prices[0].text().as_str(), "9.99");
//! ```
//!
//! # Fetching a page
//!
//! ```no_run
//! use rust_scrapling::{Fetcher, FetcherConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = FetcherConfig::builder()
//!         .timeout(20)
//!         .stealth(true) // browser-like headers (the default)
//!         .build();
//!     let fetcher = Fetcher::new(config)?;
//!
//!     let response = fetcher.get("https://example.com").await?;
//!     println!("status: {}", response.status());
//!     if let Some(title) = response.selector().css_get("title::text") {
//!         println!("title: {}", title);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # A minimal spider
//!
//! ```no_run
//! use async_trait::async_trait;
//! use rust_scrapling::spiders::response::SpiderResponse;
//! use rust_scrapling::spiders::session::SessionManager;
//! use rust_scrapling::{CrawlerEngine, FetcherConfig, Spider, SpiderRequest};
//! use std::sync::Arc;
//!
//! struct QuotesSpider;
//!
//! #[async_trait]
//! impl Spider for QuotesSpider {
//!     fn name(&self) -> &str {
//!         "quotes"
//!     }
//!
//!     fn start_urls(&self) -> Vec<String> {
//!         vec!["https://quotes.toscrape.com/".to_string()]
//!     }
//!
//!     async fn parse(
//!         &self,
//!         response: SpiderResponse,
//!     ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
//!         let items = response
//!             .css_getall(".quote .text::text")
//!             .into_iter()
//!             .map(|text| serde_json::json!({ "quote": text.as_str() }))
//!             .collect();
//!         (items, Vec::new()) // no follow-up requests
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let sessions = SessionManager::new(FetcherConfig::default());
//!     let engine = CrawlerEngine::new(Arc::new(QuotesSpider), sessions, None)
//!         .expect("HTTP client should build");
//!     let result = engine.crawl().await;
//!     println!("scraped {} items", result.items.len());
//! }
//! ```
//!
//! # Feature highlights
//!
//! - CSS selectors with `::text` / `::attr(name)` extraction
//!   ([`Selector::css_get`] / [`Selector::css_getall`])
//! - Adaptive element relocation that survives page-layout changes
//!   ([`parser::adaptive`])
//! - Stealth (browser-like) request headers, on by default
//!   ([`FetcherConfig::build_headers`])
//! - robots.txt compliance with RFC 9309 longest-match rules and
//!   `Crawl-delay` support ([`spiders::robots`])
//! - AutoThrottle: adaptive per-domain crawl delays ([`spiders::throttle`])
//! - Checkpoints with pause/resume ([`spiders::checkpoint`],
//!   [`CrawlerEngine::request_pause`])
//! - Development-mode response cache for offline replays
//!   ([`spiders::cache`])
//! - Item export to CSV / JSON / JSON Lines / XML ([`ItemList`])

#![warn(missing_docs)]

pub mod core;
pub mod fetchers;
pub mod parser;
pub mod spiders;

// Re-export primary types at crate root
pub use fetchers::client::Fetcher;
pub use fetchers::config::FetcherConfig;
pub use fetchers::response::Response;
pub use parser::{Selector, Selectors};
pub use spiders::engine::CrawlerEngine;
pub use spiders::links::LinkExtractor;
pub use spiders::request::SpiderRequest;
pub use spiders::result::{CrawlResult, CrawlStats, ItemList};
pub use spiders::spider::Spider;
pub use spiders::templates::{CrawlRule, CrawlSpider, ShopifySpider, SitemapSpider};
