use crate::parser::Selector;
use std::collections::HashMap;

/// HTTP response wrapper with parser integration.
#[derive(Debug, Clone)]
pub struct Response {
    status_code: u16,
    content_type: String,
    body: String,
    url: String,
    headers: HashMap<String, String>,
}

impl Response {
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

    pub fn status(&self) -> u16 {
        self.status_code
    }

    pub fn text(&self) -> &str {
        &self.body
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn content_length(&self) -> usize {
        self.body.len()
    }

    /// Parse response body as Selector for HTML extraction.
    pub fn selector(&self) -> Selector {
        Selector::from_html_with_url(&self.body, &self.url)
    }

    /// Parse body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.body)
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    pub fn is_blocked(&self) -> bool {
        crate::fetchers::constants::BLOCKED_STATUS_CODES.contains(&self.status_code)
    }
}
