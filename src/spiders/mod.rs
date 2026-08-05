//! Crawling: the [`spider::Spider`] trait, the [`engine::CrawlerEngine`]
//! that runs it, and the supporting machinery — scheduling with request
//! dedup, robots.txt compliance, AutoThrottle, checkpoints for
//! pause/resume, a development-mode response cache, link extraction, and
//! crawl results/exports.

pub mod cache;
pub mod checkpoint;
pub mod engine;
pub mod links;
pub mod request;
pub mod response;
pub mod result;
pub mod robots;
pub mod scheduler;
pub mod session;
pub mod spider;
pub mod templates;
pub mod throttle;
