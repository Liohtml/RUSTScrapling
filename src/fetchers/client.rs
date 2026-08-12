//! The [`Fetcher`] HTTP client: retries, stealth headers, proxy rotation,
//! redirect policy, and response-size protection on top of reqwest.

use crate::fetchers::config::FetcherConfig;
use crate::fetchers::proxy::ProxyRotator;
use crate::fetchers::response::Response;
use std::collections::HashMap;
use std::time::Duration;

/// Async HTTP client configured through [`FetcherConfig`].
///
/// Every request applies the config's headers (with browser-like stealth
/// headers when enabled), retries failed sends up to `config.retries`
/// times with a fixed delay between attempts, enforces the configured
/// response-size cap while streaming the body, and decodes the body using
/// the charset advertised in `Content-Type`. With `proxy_list` set, one
/// client is built per proxy and requests rotate through them round-robin.
pub struct Fetcher {
    config: FetcherConfig,
    /// One client per rotating proxy when rotation is enabled, otherwise a
    /// single client. Indexed by `rotator` when present.
    clients: Vec<reqwest::Client>,
    rotator: Option<ProxyRotator>,
}

/// Errors from building a [`Fetcher`] or performing a request.
///
/// Variants are typed (rather than stringified) so callers can build retry
/// policies: a timeout or connect failure is worth retrying, a body-size
/// violation is not. Marked `#[non_exhaustive]` so future variants are not
/// a breaking change — match with a wildcard arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FetcherError {
    /// Every send attempt failed at the transport level (DNS, connect,
    /// TLS, timeout, …). `source` is the LAST attempt's error; classify it
    /// with [`FetcherError::is_timeout`] / [`FetcherError::is_connect`] or
    /// inspect the [`reqwest::Error`] directly.
    #[error("request failed after {attempts} attempt(s): {source}")]
    ExhaustedRetries {
        /// Total attempts made (initial try + retries).
        attempts: u32,
        /// The last attempt's transport error.
        #[source]
        source: reqwest::Error,
    },
    /// The response body exceeded the configured size cap
    /// (`FetcherConfig::max_body_bytes`). Not retried — the same URL will
    /// exceed the cap again.
    #[error("response body exceeds the {limit_bytes}-byte cap")]
    TooLarge {
        /// The configured cap.
        limit_bytes: usize,
        /// The server-advertised `Content-Length`, when the violation was
        /// detected up front; `None` when the cap was hit mid-stream.
        advertised_bytes: Option<u64>,
    },
    /// A body chunk could not be read after the response headers arrived.
    #[error("body read failed: {0}")]
    BodyRead(#[source] reqwest::Error),
    /// The underlying reqwest client failed to build (e.g. TLS backend
    /// initialization).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

impl FetcherError {
    /// The underlying transport error, when this error wraps one.
    #[must_use]
    pub fn transport_error(&self) -> Option<&reqwest::Error> {
        match self {
            FetcherError::ExhaustedRetries { source, .. } => Some(source),
            FetcherError::BodyRead(e) | FetcherError::Http(e) => Some(e),
            _ => None,
        }
    }

    /// Whether the underlying failure was a timeout (worth retrying).
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        self.transport_error()
            .is_some_and(reqwest::Error::is_timeout)
    }

    /// Whether the underlying failure was a connection failure (DNS,
    /// refused, unreachable — often worth retrying or rotating a proxy).
    #[must_use]
    pub fn is_connect(&self) -> bool {
        self.transport_error()
            .is_some_and(reqwest::Error::is_connect)
    }
}

impl Fetcher {
    /// Build a fetcher from config.
    ///
    /// Returns [`FetcherError::Http`] if the underlying reqwest client cannot
    /// be built (e.g. the TLS backend fails to initialize on a misconfigured
    /// system) rather than panicking, so callers can handle it.
    pub fn new(config: FetcherConfig) -> Result<Self, FetcherError> {
        if !config.verify_ssl {
            log::warn!(
                "SSL certificate verification is DISABLED. This is insecure \
                 and must not be used in production."
            );
        }

        let (clients, rotator) = if config.proxy_list.is_empty() {
            // No rotation: a single client honouring `proxy` and the
            // per-protocol `proxies` map.
            (vec![Self::build_client(&config, None)?], None)
        } else {
            // Rotation: one client bound to each proxy, selected round-robin.
            let clients = config
                .proxy_list
                .iter()
                .map(|p| Self::build_client(&config, Some(p)))
                .collect::<Result<Vec<_>, _>>()?;
            let rotator = ProxyRotator::new(config.proxy_list.clone());
            (clients, rotator)
        };

        Ok(Self {
            config,
            clients,
            rotator,
        })
    }

    /// Build a single reqwest client. When `proxy_override` is `Some`, that
    /// proxy is applied for all protocols; otherwise the config's `proxy` and
    /// per-protocol `proxies` map are applied.
    fn build_client(
        config: &FetcherConfig,
        proxy_override: Option<&str>,
    ) -> Result<reqwest::Client, FetcherError> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .danger_accept_invalid_certs(!config.verify_ssl);

        // Configure redirects
        if config.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::limited(
                config.max_redirects as usize,
            ));
        } else {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        // Configure proxy
        if let Some(proxy_url) = proxy_override {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        } else {
            // Apply scheme-specific proxies before any wildcard so the specific
            // ones win (reqwest uses the first matching proxy). `proxies` is a
            // HashMap, so iterate in a deterministic order.
            if let Some(proxy_url) = config.proxies.get("http") {
                if let Ok(proxy) = reqwest::Proxy::http(proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
            if let Some(proxy_url) = config.proxies.get("https") {
                if let Ok(proxy) = reqwest::Proxy::https(proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
            let mut wildcard_keys: Vec<&String> = config
                .proxies
                .keys()
                .filter(|k| k.as_str() != "http" && k.as_str() != "https")
                .collect();
            wildcard_keys.sort();
            for key in wildcard_keys {
                if let Ok(proxy) = reqwest::Proxy::all(&config.proxies[key]) {
                    builder = builder.proxy(proxy);
                }
            }
            // Single wildcard proxy applied last as a general fallback.
            if let Some(ref proxy_url) = config.proxy {
                if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
        }

        Ok(builder.build()?)
    }

    /// Select the client to use for the next request attempt. With rotation
    /// enabled this advances the round-robin cursor so a failing proxy is
    /// swapped on retry.
    fn next_client(&self) -> &reqwest::Client {
        match &self.rotator {
            Some(rotator) => &self.clients[rotator.next_index()],
            None => &self.clients[0],
        }
    }

    /// Send a GET request.
    pub async fn get(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::GET, url, None, None, None)
            .await
    }

    /// Send a POST request with an optional plain-text `body` or a `json`
    /// payload (which also sets `Content-Type: application/json`). When
    /// both are given, `json` wins.
    pub async fn post(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::POST, url, body, json, None)
            .await
    }

    /// Send a PUT request with an optional plain-text `body` or a `json`
    /// payload (which also sets `Content-Type: application/json`). When
    /// both are given, `json` wins.
    pub async fn put(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::PUT, url, body, json, None)
            .await
    }

    /// Send a DELETE request.
    pub async fn delete(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::DELETE, url, None, None, None)
            .await
    }

    /// Internal request method with retry loop. `extra_headers`, when set,
    /// are applied on top of the config-built headers (e.g. per-request
    /// headers from a [`crate::spiders::request::SpiderRequest`]) — used by
    /// [`crate::spiders::session::SessionManager`] so per-request headers set
    /// via `SpiderRequestBuilder::header` actually reach the wire.
    pub(crate) async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response, FetcherError> {
        let mut headers = self.config.build_headers(url, self.config.stealthy_headers);
        if let Some(extra) = extra_headers {
            for (k, v) in extra {
                headers.insert(k.clone(), v.clone());
            }
        }

        let mut last_error: Option<reqwest::Error> = None;

        for attempt in 0..=self.config.retries {
            let attempt_started = std::time::Instant::now();
            let mut req = self.next_client().request(method.clone(), url);

            // Set headers
            for (key, value) in &headers {
                req = req.header(key.as_str(), value.as_str());
            }

            // Set body or json
            if let Some(json_val) = json {
                req = req.json(json_val);
            } else if let Some(text_body) = body {
                req = req.body(text_body.to_string());
            }

            match req.send().await {
                Ok(mut resp) => {
                    let status_code = resp.status().as_u16();
                    let final_url = resp.url().to_string();

                    // Extract headers
                    let mut resp_headers: HashMap<String, String> = HashMap::new();
                    for (key, value) in resp.headers() {
                        if let Ok(v) = value.to_str() {
                            resp_headers.insert(key.to_string(), v.to_string());
                        }
                    }

                    // Extract content-type before consuming response
                    let content_type = resp_headers
                        .get("content-type")
                        .cloned()
                        .unwrap_or_default();

                    // Reject obviously-too-large bodies up front when the
                    // server advertised a Content-Length. Use try_from so the
                    // u64 -> usize cast cannot truncate on 32-bit targets.
                    let max_body = self.config.max_body_bytes;
                    if let Some(len) = resp.content_length() {
                        let too_large = match usize::try_from(len) {
                            Ok(n) => n > max_body,
                            Err(_) => true,
                        };
                        if too_large {
                            return Err(FetcherError::TooLarge {
                                limit_bytes: max_body,
                                advertised_bytes: Some(len),
                            });
                        }
                    }

                    // Stream the body and cap the accumulated size so a
                    // server that lies about (or omits) Content-Length still
                    // cannot exhaust memory. Chunk read errors surface as a
                    // request failure rather than a truncated success.
                    let mut bytes: Vec<u8> = Vec::new();
                    loop {
                        match resp.chunk().await {
                            Ok(Some(chunk)) => {
                                let new_len = bytes.len().checked_add(chunk.len());
                                if new_len.map(|n| n > max_body).unwrap_or(true) {
                                    return Err(FetcherError::TooLarge {
                                        limit_bytes: max_body,
                                        advertised_bytes: None,
                                    });
                                }
                                bytes.extend_from_slice(&chunk);
                            }
                            Ok(None) => break,
                            Err(e) => {
                                return Err(FetcherError::BodyRead(e));
                            }
                        }
                    }
                    let body_text = crate::fetchers::encoding::decode_body(&bytes, &content_type);

                    let mut response = Response::new(
                        status_code,
                        content_type,
                        body_text,
                        final_url,
                        resp_headers,
                    );
                    // Latency of THIS attempt only (send + body streaming),
                    // excluding earlier failed attempts and retry sleeps —
                    // AutoThrottle adapts to how the server actually
                    // responded, not to how flaky the path there was.
                    response.set_latency(attempt_started.elapsed());
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.retries {
                        tokio::time::sleep(Duration::from_secs(self.config.retry_delay_secs)).await;
                    }
                }
            }
        }

        Err(FetcherError::ExhaustedRetries {
            attempts: self.config.retries + 1,
            source: last_error.expect("loop ran at least once, so a send error was recorded"),
        })
    }
}
