use crate::fetchers::response::Response as FetcherResponse;
use crate::parser::{Selector, Selectors};

pub struct SpiderResponse {
    inner: FetcherResponse,
}

impl SpiderResponse {
    pub fn new(inner: FetcherResponse) -> Self {
        Self { inner }
    }

    /// The underlying fetcher response (full headers, content type, body).
    pub fn response(&self) -> &FetcherResponse {
        &self.inner
    }

    pub fn status(&self) -> u16 {
        self.inner.status()
    }

    pub fn text(&self) -> &str {
        self.inner.text()
    }

    pub fn url(&self) -> &str {
        self.inner.url()
    }

    pub fn selector(&self) -> Selector {
        self.inner.selector()
    }

    pub fn css(&self, selector: &str) -> Selectors {
        self.selector().css(selector)
    }

    /// Parsel-style CSS query with `::text` / `::attr(name)` support; all
    /// extracted values. See [`Selector::css_getall`].
    pub fn css_getall(&self, query: &str) -> Vec<crate::core::text_handler::TextHandler> {
        self.selector().css_getall(query)
    }

    /// Parsel-style CSS query with `::text` / `::attr(name)` support; first
    /// extracted value. See [`Selector::css_get`].
    pub fn css_get(&self, query: &str) -> Option<crate::core::text_handler::TextHandler> {
        self.selector().css_get(query)
    }

    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        self.inner.json()
    }

    pub fn is_blocked(&self) -> bool {
        self.inner.is_blocked()
    }

    pub fn content_length(&self) -> usize {
        self.inner.content_length()
    }
}
