//! RUSTScrapling - A Rust port of the Scrapling web scraping framework.

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
pub use spiders::request::SpiderRequest;
pub use spiders::result::{CrawlResult, CrawlStats, ItemList};
pub use spiders::spider::Spider;
