use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::cmp::Ordering;
use url::Url;

#[derive(Debug, Clone)]
pub struct SpiderRequest {
    url: String,
    method: String,
    session_id: String,
    callback_name: Option<String>,
    priority: i32,
    dont_filter: bool,
    meta: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
    body: Option<String>,
    retry_count: u32,
    fingerprint: String,
}

impl SpiderRequest {
    pub fn new(url: &str) -> Self {
        let mut req = Self {
            url: url.to_string(),
            method: "GET".to_string(),
            session_id: String::new(),
            callback_name: None,
            priority: 0,
            dont_filter: false,
            meta: HashMap::new(),
            headers: HashMap::new(),
            body: None,
            retry_count: 0,
            fingerprint: String::new(),
        };
        req.fingerprint = req.compute_fingerprint(false, false, false);
        req
    }

    pub fn builder(url: &str) -> SpiderRequestBuilder {
        SpiderRequestBuilder::new(url)
    }

    // Getters
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn dont_filter(&self) -> bool {
        self.dont_filter
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn callback_name(&self) -> Option<&str> {
        self.callback_name.as_deref()
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn meta(&self, key: &str) -> Option<&serde_json::Value> {
        self.meta.get(key)
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn domain(&self) -> Option<String> {
        Url::parse(&self.url).ok().and_then(|u| u.host_str().map(|h| h.to_string()))
    }

    // Setters
    pub fn set_dont_filter(&mut self, val: bool) {
        self.dont_filter = val;
    }

    pub fn set_session_id(&mut self, session_id: &str) {
        self.session_id = session_id.to_string();
    }

    pub fn set_meta(&mut self, key: &str, value: serde_json::Value) {
        self.meta.insert(key.to_string(), value);
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn update_fingerprint(
        &mut self,
        include_kwargs: bool,
        include_headers: bool,
        keep_fragments: bool,
    ) {
        self.fingerprint = self.compute_fingerprint(include_kwargs, include_headers, keep_fragments);
    }

    fn compute_fingerprint(
        &self,
        include_kwargs: bool,
        include_headers: bool,
        keep_fragments: bool,
    ) -> String {
        let mut hasher = Sha256::new();

        hasher.update(self.session_id.as_bytes());
        hasher.update(self.method.as_bytes());

        // Canonical URL
        let canonical = if let Ok(mut parsed) = Url::parse(&self.url) {
            if !keep_fragments {
                parsed.set_fragment(None);
            }
            parsed.to_string()
        } else {
            self.url.clone()
        };
        hasher.update(canonical.as_bytes());

        if let Some(ref body) = self.body {
            hasher.update(body.as_bytes());
        }

        if include_headers {
            let mut keys: Vec<&String> = self.headers.keys().collect();
            keys.sort();
            for k in keys {
                hasher.update(k.as_bytes());
                hasher.update(self.headers[k].as_bytes());
            }
        }

        if include_kwargs {
            let mut keys: Vec<&String> = self.meta.keys().collect();
            keys.sort();
            for k in keys {
                hasher.update(k.as_bytes());
                hasher.update(self.meta[k].to_string().as_bytes());
            }
        }

        format!("{:x}", hasher.finalize())
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }
}

impl PartialEq for SpiderRequest {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

impl Eq for SpiderRequest {}

impl PartialOrd for SpiderRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SpiderRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

// Builder
pub struct SpiderRequestBuilder {
    url: String,
    method: String,
    session_id: String,
    callback_name: Option<String>,
    priority: i32,
    dont_filter: bool,
    meta: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl SpiderRequestBuilder {
    fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".to_string(),
            session_id: String::new(),
            callback_name: None,
            priority: 0,
            dont_filter: false,
            meta: HashMap::new(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    pub fn callback(mut self, name: &str) -> Self {
        self.callback_name = Some(name.to_string());
        self
    }

    pub fn dont_filter(mut self, val: bool) -> Self {
        self.dont_filter = val;
        self
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub fn meta(mut self, key: &str, value: serde_json::Value) -> Self {
        self.meta.insert(key.to_string(), value);
        self
    }

    pub fn build(self) -> SpiderRequest {
        let mut req = SpiderRequest {
            url: self.url,
            method: self.method,
            session_id: self.session_id,
            callback_name: self.callback_name,
            priority: self.priority,
            dont_filter: self.dont_filter,
            meta: self.meta,
            headers: self.headers,
            body: self.body,
            retry_count: 0,
            fingerprint: String::new(),
        };
        req.fingerprint = req.compute_fingerprint(false, false, false);
        req
    }
}
