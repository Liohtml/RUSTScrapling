//! [`SpiderResponse`]: the response type handed to
//! [`Spider::parse`](crate::spiders::spider::Spider::parse), wrapping a
//! fetcher [`Response`](crate::fetchers::response::Response) with direct
//! extraction shortcuts.

use crate::fetchers::response::Response as FetcherResponse;
use crate::parser::{Selector, Selectors};

/// A fetched (or cache-replayed) response as seen by a spider's `parse`
/// callback, with parsing shortcuts. Note each `css*` call re-parses the
/// body; when making many queries, call [`SpiderResponse::selector`] once
/// and query that.
pub struct SpiderResponse {
    inner: FetcherResponse,
}

impl SpiderResponse {
    /// Wrap a fetcher response.
    pub fn new(inner: FetcherResponse) -> Self {
        Self { inner }
    }

    /// The underlying fetcher response (full headers, content type, body).
    #[must_use]
    pub fn response(&self) -> &FetcherResponse {
        &self.inner
    }

    /// The HTTP status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.inner.status()
    }

    /// The decoded response body.
    #[must_use]
    pub fn text(&self) -> &str {
        self.inner.text()
    }

    /// The final URL of the response, after any redirects.
    #[must_use]
    pub fn url(&self) -> &str {
        self.inner.url()
    }

    /// Parse the body as HTML into a [`Selector`] rooted at the document
    /// (base URL set for `urljoin`).
    #[must_use]
    pub fn selector(&self) -> Selector {
        self.inner.selector()
    }

    /// Parse the body and run a plain CSS query against it (see
    /// [`Selector::css`]).
    #[must_use]
    pub fn css(&self, selector: &str) -> Selectors {
        self.selector().css(selector)
    }

    /// Parsel-inspired CSS query with `::text` / `::attr(name)` support; all
    /// extracted values. See [`Selector::css_getall`].
    #[must_use]
    pub fn css_getall(&self, query: &str) -> Vec<crate::core::text_handler::TextHandler> {
        self.selector().css_getall(query)
    }

    /// Parsel-inspired CSS query with `::text` / `::attr(name)` support; first
    /// extracted value. See [`Selector::css_get`].
    #[must_use]
    pub fn css_get(&self, query: &str) -> Option<crate::core::text_handler::TextHandler> {
        self.selector().css_get(query)
    }

    /// Parse the body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        self.inner.json()
    }

    /// Whether the status code is one of the bot-blocking codes (see
    /// [`Response::is_blocked`](crate::fetchers::response::Response::is_blocked)).
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.inner.is_blocked()
    }

    /// Size of the decoded body in bytes.
    #[must_use]
    pub fn content_length(&self) -> usize {
        self.inner.content_length()
    }
}
