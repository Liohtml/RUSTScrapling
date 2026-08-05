//! The [`Response`] wrapper returned by the
//! [`Fetcher`](crate::fetchers::client::Fetcher): status, headers, decoded
//! body, and one-call access to the parser.

use crate::parser::Selector;
use std::collections::HashMap;

/// HTTP response wrapper with parser integration.
///
/// The body is already fully read and decoded to a `String` (using the
/// charset from the `Content-Type` header), so all accessors are cheap and
/// synchronous.
#[derive(Debug, Clone)]
pub struct Response {
    status_code: u16,
    content_type: String,
    body: String,
    url: String,
    headers: HashMap<String, String>,
}

impl Response {
    /// Assemble a response from its parts (used by the fetcher and by the
    /// dev-mode cache when replaying stored responses).
    pub fn new(
        status_code: u16,
        content_type: String,
        body: String,
        url: String,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            status_code,
            content_type,
            body,
            url,
            headers,
        }
    }

    /// The HTTP status code (e.g. `200`).
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status_code
    }

    /// The decoded response body.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.body
    }

    /// The final URL of the response, after any redirects.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The raw `Content-Type` header value (empty when the server sent
    /// none).
    #[must_use]
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// All response headers, with lowercase names. Headers whose values are
    /// not valid UTF-8 are omitted.
    #[must_use]
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Size of the *decoded* body in bytes (not the on-wire
    /// `Content-Length`, which may differ after decompression and charset
    /// decoding).
    #[must_use]
    pub fn content_length(&self) -> usize {
        self.body.len()
    }

    /// Parse the body as HTML and return a [`Selector`] rooted at the
    /// document, with the response URL as base for
    /// [`Selector::urljoin`]. Each call re-parses the body.
    #[must_use]
    pub fn selector(&self) -> Selector {
        Selector::from_html_with_url(&self.body, &self.url)
    }

    /// Parse the body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.body)
    }

    /// Whether the status code is 2xx.
    #[must_use]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// Whether the status code is one of the bot-blocking codes
    /// (401/403/407/429/444 — see
    /// [`BLOCKED_STATUS_CODES`](crate::fetchers::constants::BLOCKED_STATUS_CODES)).
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        crate::fetchers::constants::BLOCKED_STATUS_CODES.contains(&self.status_code)
    }
}
