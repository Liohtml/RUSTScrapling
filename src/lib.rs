//! RUSTScrapling - A Rust port of the Scrapling web scraping framework.

pub mod core;
pub mod parser;
pub mod fetchers;
pub mod spiders;

// Re-export primary types at crate root
pub use parser::{Selector, Selectors};
pub use fetchers::client::Fetcher;
pub use fetchers::config::FetcherConfig;
pub use fetchers::response::Response;
pub use spiders::spider::Spider;
pub use spiders::request::SpiderRequest;
pub use spiders::result::{CrawlResult, CrawlStats, ItemList};
pub use spiders::engine::CrawlerEngine;
