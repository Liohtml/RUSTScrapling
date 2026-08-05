//! Async HTTP fetching: a reqwest-backed client ([`client::Fetcher`]) with
//! stealth headers, retries, proxy support (single, per-protocol, and
//! rotating), charset-aware body decoding, and response wrappers that plug
//! straight into the parser.

pub mod client;
pub mod config;
pub mod constants;
pub mod encoding;
pub mod proxy;
pub mod response;
pub mod search;
