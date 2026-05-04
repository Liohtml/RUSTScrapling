use crate::fetchers::response::Response as FetcherResponse;
use crate::parser::{Selector, Selectors};

pub struct SpiderResponse {
    inner: FetcherResponse,
}

impl SpiderResponse {
    pub fn new(inner: FetcherResponse) -> Self {
        Self { inner }
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
