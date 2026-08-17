//! Generic spider templates that build on the [`Spider`](crate::spiders::spider::Spider) trait.

pub mod crawler;
pub mod csv_feed;
pub mod shopify;
pub mod sitemap;
pub mod xml_feed;

pub use crawler::{CrawlRule, CrawlSpider};
pub use csv_feed::CsvFeedSpider;
pub use shopify::ShopifySpider;
pub use sitemap::SitemapSpider;
pub use xml_feed::XmlFeedSpider;
