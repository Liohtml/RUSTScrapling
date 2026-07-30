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
    #[error("session '{0}' not found")]
    NotFound(String),
    #[error("unsupported HTTP method: {0}")]
    UnsupportedMethod(String),
    #[error("network error: {0}")]
    Network(String),
}

pub struct SessionManager {
    sessions: HashMap<String, Fetcher>,
    default_config: FetcherConfig,
}

impl SessionManager {
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
