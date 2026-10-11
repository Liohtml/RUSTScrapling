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
    latency: std::time::Duration,
}

impl Response {
    /// Assemble a response from its parts (used by the fetcher and by the
    /// dev-mode cache when replaying stored responses). Latency starts at
    /// zero; the fetcher stamps the real value on live responses.
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
            latency: std::time::Duration::ZERO,
        }
    }

    /// Wall-clock time of the successful fetch attempt (request sent until
    /// the body finished streaming). Excludes earlier failed attempts and
    /// retry sleeps. Zero for responses not produced by a live fetch (e.g.
    /// dev-cache replays or hand-assembled responses).
    #[must_use]
    pub fn latency(&self) -> std::time::Duration {
        self.latency
    }

    pub(crate) fn set_latency(&mut self, latency: std::time::Duration) {
        self.latency = latency;
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

    /// Convert the response content to clean, LLM-ready Markdown.
    ///
    /// Scripts, styles, and hidden / prompt-injection content are always
    /// removed before conversion; see the
    /// [`markdown`](crate::parser::markdown) module docs for details.
    ///
    /// With `css_selector`, only the matching elements are converted and
    /// the results concatenated. With `main_content_only`, only the content
    /// inside the page's `<body>` tag is converted.
    #[must_use]
    pub fn markdown(&self, css_selector: Option<&str>, main_content_only: bool) -> String {
        let root = self.selector();
        let page = if main_content_only {
            root.css("body").into_iter().next().unwrap_or(root)
        } else {
            root
        };
        match css_selector {
            None => page.markdown().to_string(),
            Some(sel) => {
                // Matches sitting inside hidden/noise ancestors are
                // rejected: converting such a subtree from its own root
                // would resurrect content the sanitizer guarantees to
                // strip (upstream drops hidden subtrees before selecting).
                let parts: Vec<String> = page
                    .css(sel)
                    .into_iter()
                    .filter(|el| !crate::parser::markdown::in_hidden_context(el))
                    .map(|el| el.markdown().to_string())
                    .collect();
                parts.join("\n")
            }
        }
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
