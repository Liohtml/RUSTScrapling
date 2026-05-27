use crate::fetchers::config::FetcherConfig;
use crate::fetchers::proxy::ProxyRotator;
use crate::fetchers::response::Response;
use std::collections::HashMap;
use std::time::Duration;

pub struct Fetcher {
    config: FetcherConfig,
    /// One client per rotating proxy when rotation is enabled, otherwise a
    /// single client. Indexed by `rotator` when present.
    clients: Vec<reqwest::Client>,
    rotator: Option<ProxyRotator>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetcherError {
    #[error("Request failed after retries: {0}")]
    RequestFailed(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

impl Fetcher {
    pub fn new(config: FetcherConfig) -> Self {
        let (clients, rotator) = if config.proxy_list.is_empty() {
            // No rotation: a single client honouring `proxy` and the
            // per-protocol `proxies` map.
            (vec![Self::build_client(&config, None)], None)
        } else {
            // Rotation: one client bound to each proxy, selected round-robin.
            let clients = config
                .proxy_list
                .iter()
                .map(|p| Self::build_client(&config, Some(p)))
                .collect();
            let rotator = ProxyRotator::new(config.proxy_list.clone());
            (clients, rotator)
        };

        Self {
            config,
            clients,
            rotator,
        }
    }

    /// Build a single reqwest client. When `proxy_override` is `Some`, that
    /// proxy is applied for all protocols; otherwise the config's `proxy` and
    /// per-protocol `proxies` map are applied.
    fn build_client(config: &FetcherConfig, proxy_override: Option<&str>) -> reqwest::Client {
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

        builder.build().expect("Failed to build reqwest client")
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

    pub async fn get(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::GET, url, None, None).await
    }

    pub async fn post(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::POST, url, body, json).await
    }

    pub async fn put(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::PUT, url, body, json).await
    }

    pub async fn delete(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::DELETE, url, None, None).await
    }

    /// Internal request method with retry loop.
    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        let headers = self.config.build_headers(url, self.config.stealthy_headers);

        let mut last_error = String::new();

        for attempt in 0..=self.config.retries {
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
                Ok(resp) => {
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

                    let body_text = resp.text().await.unwrap_or_default();

                    return Ok(Response::new(
                        status_code,
                        content_type,
                        body_text,
                        final_url,
                        resp_headers,
                    ));
                }
                Err(e) => {
                    last_error = e.to_string();
                    if attempt < self.config.retries {
                        tokio::time::sleep(Duration::from_secs(self.config.retry_delay_secs)).await;
                    }
                }
            }
        }

        Err(FetcherError::RequestFailed(last_error))
    }
}
