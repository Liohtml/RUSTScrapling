//! The [`SessionManager`]: named [`Fetcher`] instances (each with its own
//! config, cookies, and proxies) that spider requests are routed through
//! via their `session_id`.

use crate::fetchers::client::{Fetcher, FetcherError};
use crate::fetchers::config::FetcherConfig;
use crate::fetchers::response::Response;
use crate::spiders::request::SpiderRequest;
use std::collections::HashMap;

/// Distinct failure modes for [`SessionManager::fetch`], so callers can tell a
/// configuration error (wrong session name → not recoverable, abort) apart
/// from a transient network failure (worth retrying).
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// The request named a session that was never added.
    #[error("session '{0}' not found")]
    NotFound(String),
    /// The request's HTTP method is not one of GET/POST/PUT/DELETE.
    #[error("unsupported HTTP method: {0}")]
    UnsupportedMethod(String),
    /// The fetch itself failed (after the fetcher's own retries).
    #[error("network error: {0}")]
    Network(String),
}

/// A registry of named [`Fetcher`] sessions.
///
/// The [`CrawlerEngine`](crate::spiders::engine::CrawlerEngine) fetches
/// each [`SpiderRequest`] through the session named by its `session_id`
/// (or `"default"`, created lazily from the default config). Multiple
/// sessions let one crawl use different configurations — e.g. distinct
/// proxies, headers, or cookie jars — for different requests.
pub struct SessionManager {
    sessions: HashMap<String, Fetcher>,
    default_config: FetcherConfig,
}

impl SessionManager {
    /// Create a manager with no sessions yet; `default_config` is used to
    /// build the `"default"` session on demand.
    pub fn new(default_config: FetcherConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            default_config,
        }
    }

    /// Add a named session. Propagates the error if the underlying HTTP client
    /// cannot be built (see [`Fetcher::new`]).
    pub fn add_session(&mut self, name: &str, config: FetcherConfig) -> Result<(), FetcherError> {
        self.sessions
            .insert(name.to_string(), Fetcher::new(config)?);
        Ok(())
    }

    /// Ensure a "default" session exists, building it from the default config.
    pub fn ensure_default(&mut self) -> Result<(), FetcherError> {
        if !self.sessions.contains_key("default") {
            let fetcher = Fetcher::new(self.default_config.clone())?;
            self.sessions.insert("default".to_string(), fetcher);
        }
        Ok(())
    }

    /// Fetch using the session specified in the request (or "default").
    pub async fn fetch(&self, request: &SpiderRequest) -> Result<Response, SessionError> {
        let session_id = if request.session_id().is_empty() {
            "default"
        } else {
            request.session_id()
        };
        let fetcher = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        // Per-request headers (set via `SpiderRequestBuilder::header`) are
        // layered on top of the session's config-built headers.
        let extra_headers = if request.headers().is_empty() {
            None
        } else {
            Some(request.headers())
        };

        let result = match request.method() {
            "GET" => {
                fetcher
                    .request(
                        reqwest::Method::GET,
                        request.url(),
                        None,
                        None,
                        extra_headers,
                    )
                    .await
            }
            "POST" => {
                fetcher
                    .request(
                        reqwest::Method::POST,
                        request.url(),
                        request.body(),
                        None,
                        extra_headers,
                    )
                    .await
            }
            "PUT" => {
                fetcher
                    .request(
                        reqwest::Method::PUT,
                        request.url(),
                        request.body(),
                        None,
                        extra_headers,
                    )
                    .await
            }
            "DELETE" => {
                fetcher
                    .request(
                        reqwest::Method::DELETE,
                        request.url(),
                        None,
                        None,
                        extra_headers,
                    )
                    .await
            }
            m => return Err(SessionError::UnsupportedMethod(m.to_string())),
        };
        result.map_err(|e| SessionError::Network(e.to_string()))
    }
}
