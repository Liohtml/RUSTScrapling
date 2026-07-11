//! Generic spider templates that build on the [`Spider`](crate::spiders::spider::Spider) trait.

pub mod crawler;
pub mod sitemap;

pub use crawler::{CrawlRule, CrawlSpider};
pub use sitemap::SitemapSpider;
