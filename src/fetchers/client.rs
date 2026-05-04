use crate::fetchers::config::FetcherConfig;
use crate::fetchers::response::Response;
use std::collections::HashMap;
use std::time::Duration;

pub struct Fetcher {
    config: FetcherConfig,
    client: reqwest::Client,
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
        if let Some(ref proxy_url) = config.proxy {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }

        let client = builder
            .build()
            .expect("Failed to build reqwest client");

        Self { config, client }
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
            let mut req = self.client.request(method.clone(), url);

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
                        tokio::time::sleep(Duration::from_secs(self.config.retry_delay_secs))
                            .await;
                    }
                }
            }
        }

        Err(FetcherError::RequestFailed(last_error))
    }
}
