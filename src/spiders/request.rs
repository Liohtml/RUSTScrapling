//! [`SpiderRequest`]: one unit of work for the crawler, carrying the URL,
//! method, headers/body, priority, per-request metadata, and the SHA-256
//! fingerprint used for deduplication.

use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::HashMap;
use url::Url;

/// A request scheduled for crawling.
///
/// Create simple GET requests with [`SpiderRequest::new`], or use
/// [`SpiderRequest::builder`] for anything more (method, headers, body,
/// priority, session, metadata). Requests are ordered by `priority`
/// (higher first) in the scheduler's queue, and compared equal when their
/// dedup fingerprints match. Serializable so pending requests survive
/// checkpoints intact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Create a plain GET request for `url` with default settings
    /// (priority 0, no headers, dedup-filtered).
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

    /// Start building a request with non-default method, headers, body,
    /// priority, session, or metadata.
    pub fn builder(url: &str) -> SpiderRequestBuilder {
        SpiderRequestBuilder::new(url)
    }

    // Getters

    /// The request URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The HTTP method (`"GET"`, `"POST"`, …).
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Scheduling priority; higher values are dequeued first. Default 0.
    #[must_use]
    pub fn priority(&self) -> i32 {
        self.priority
    }

    /// When `true`, this request bypasses the scheduler's duplicate
    /// filter (and is not recorded in its seen set).
    #[must_use]
    pub fn dont_filter(&self) -> bool {
        self.dont_filter
    }

    /// Name of the [`SessionManager`](crate::spiders::session::SessionManager)
    /// session to fetch through; empty means the `"default"` session.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// How many times this request was retried after being blocked.
    #[must_use]
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Optional callback label carried by the request. The engine itself
    /// routes everything through `parse`; this is for spiders that want to
    /// do their own routing.
    #[must_use]
    pub fn callback_name(&self) -> Option<&str> {
        self.callback_name.as_deref()
    }

    /// Per-request headers, layered on top of the session's configured
    /// headers at fetch time.
    #[must_use]
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// The request body, if any (sent for POST/PUT).
    #[must_use]
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// Look up one entry of the request's free-form metadata.
    #[must_use]
    pub fn meta(&self, key: &str) -> Option<&serde_json::Value> {
        self.meta.get(key)
    }

    /// The SHA-256 dedup fingerprint (hex). Computed from session, method,
    /// canonical URL, and body; headers/meta/fragments are included only
    /// when the corresponding `fp_*` spider options are on (see
    /// [`SpiderRequest::update_fingerprint`]).
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Host part of the request URL, or `None` when it cannot be parsed
    /// (relative URLs, `data:` URIs, …).
    #[must_use]
    pub fn domain(&self) -> Option<String> {
        Url::parse(&self.url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
    }

    // Setters

    /// Set whether this request bypasses the duplicate filter.
    pub fn set_dont_filter(&mut self, val: bool) {
        self.dont_filter = val;
    }

    /// Route this request through a named session.
    pub fn set_session_id(&mut self, session_id: &str) {
        self.session_id = session_id.to_string();
    }

    /// Insert (or overwrite) one metadata entry.
    pub fn set_meta(&mut self, key: &str, value: serde_json::Value) {
        self.meta.insert(key.to_string(), value);
    }

    /// Bump the blocked-retry counter by one.
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Recompute the dedup fingerprint with the given options:
    /// `include_kwargs` mixes in the metadata map, `include_headers` the
    /// headers (both key-sorted), and `keep_fragments` keeps the URL
    /// fragment instead of stripping it. The scheduler calls this on
    /// enqueue with the spider's `fp_*` settings.
    pub fn update_fingerprint(
        &mut self,
        include_kwargs: bool,
        include_headers: bool,
        keep_fragments: bool,
    ) {
        self.fingerprint =
            self.compute_fingerprint(include_kwargs, include_headers, keep_fragments);
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

    /// Alias for [`Clone::clone`], mirroring upstream Scrapling's
    /// `Request.copy()`.
    #[must_use]
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
        // Tie-break on fingerprint so `cmp` stays consistent with `PartialEq`
        // (required by the `Ord` contract: a == b iff a.cmp(b) == Equal) and
        // so same-priority requests get a deterministic, not arbitrary, order.
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.fingerprint.cmp(&other.fingerprint))
    }
}

/// Builder for [`SpiderRequest`], created via [`SpiderRequest::builder`].
#[must_use = "builders do nothing until `.build()` is called"]
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

    /// Set the HTTP method (`"GET"`, `"POST"`, `"PUT"`, `"DELETE"`; the
    /// session manager rejects anything else at fetch time).
    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    /// Set the scheduling priority (higher is dequeued first; default 0).
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Route the request through a named
    /// [`SessionManager`](crate::spiders::session::SessionManager) session.
    pub fn session_id(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    /// Attach a callback label (informational; see
    /// [`SpiderRequest::callback_name`]).
    pub fn callback(mut self, name: &str) -> Self {
        self.callback_name = Some(name.to_string());
        self
    }

    /// Let the request bypass the scheduler's duplicate filter.
    pub fn dont_filter(mut self, val: bool) -> Self {
        self.dont_filter = val;
        self
    }

    /// Add a per-request header (layered over the session's headers at
    /// fetch time).
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the request body (sent for POST/PUT).
    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    /// Add one free-form metadata entry, retrievable later via
    /// [`SpiderRequest::meta`].
    pub fn meta(mut self, key: &str, value: serde_json::Value) -> Self {
        self.meta.insert(key.to_string(), value);
        self
    }

    /// Finish the builder, computing the request's dedup fingerprint.
    #[must_use]
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
