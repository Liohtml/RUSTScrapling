use crate::fetchers::constants;
use std::collections::HashMap;

/// Configuration for HTTP fetchers.
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    pub timeout_secs: u64,
    pub retries: u32,
    pub retry_delay_secs: u64,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub verify_ssl: bool,
    pub proxy: Option<String>,
    pub proxies: HashMap<String, String>,
    /// Proxy URLs to rotate through, one HTTP client is built per entry and
    /// selected round-robin per request. Takes precedence over `proxy` /
    /// `proxies` when non-empty.
    pub proxy_list: Vec<String>,
    pub headers: HashMap<String, String>,
    pub stealthy_headers: bool,
    pub user_agent: Option<String>,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            retries: 3,
            retry_delay_secs: 1,
            follow_redirects: true,
            max_redirects: 10,
            verify_ssl: true,
            proxy: None,
            proxies: HashMap::new(),
            proxy_list: Vec::new(),
            headers: HashMap::new(),
            stealthy_headers: true,
            user_agent: None,
        }
    }
}

impl FetcherConfig {
    /// Returns a builder initialised with default values.
    pub fn builder() -> FetcherConfigBuilder {
        FetcherConfigBuilder::new()
    }

    /// Build the HTTP headers to send for a request.
    ///
    /// When `stealth` is `true` the function adds several browser-like headers
    /// that make the request harder to fingerprint as a bot.
    pub fn build_headers(&self, url: &str, stealth: bool) -> HashMap<String, String> {
        let mut headers = self.headers.clone();

        // User-Agent is always set.
        let ua = self.user_agent.clone().unwrap_or_else(|| {
            // Pick a pseudo-random user-agent based on the URL hash so that
            // the selection is deterministic for a given URL but varies across
            // different URLs.
            let idx = url
                .bytes()
                .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
                % constants::USER_AGENTS.len();
            constants::USER_AGENTS[idx].to_string()
        });
        headers.insert("user-agent".to_string(), ua);

        if stealth {
            headers
                .entry("accept".to_string())
                .or_insert_with(|| constants::ACCEPT_HEADER.to_string());

            headers
                .entry("accept-language".to_string())
                .or_insert_with(|| constants::ACCEPT_LANGUAGE.to_string());

            headers
                .entry("accept-encoding".to_string())
                .or_insert_with(|| constants::ACCEPT_ENCODING.to_string());

            headers
                .entry("sec-ch-ua".to_string())
                .or_insert_with(|| {
                    "\"Google Chrome\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\"".to_string()
                });

            headers
                .entry("sec-ch-ua-mobile".to_string())
                .or_insert_with(|| "?0".to_string());

            headers
                .entry("sec-ch-ua-platform".to_string())
                .or_insert_with(|| "\"Windows\"".to_string());

            headers
                .entry("sec-fetch-dest".to_string())
                .or_insert_with(|| "document".to_string());

            headers
                .entry("sec-fetch-mode".to_string())
                .or_insert_with(|| "navigate".to_string());

            headers
                .entry("sec-fetch-site".to_string())
                .or_insert_with(|| "none".to_string());

            headers
                .entry("sec-fetch-user".to_string())
                .or_insert_with(|| "?1".to_string());
        }

        headers
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`FetcherConfig`].
#[derive(Debug, Default)]
pub struct FetcherConfigBuilder {
    inner: FetcherConfig,
}

impl FetcherConfigBuilder {
    pub fn new() -> Self {
        Self {
            inner: FetcherConfig::default(),
        }
    }

    pub fn timeout(mut self, secs: u64) -> Self {
        self.inner.timeout_secs = secs;
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.inner.retries = retries;
        self
    }

    pub fn retry_delay(mut self, secs: u64) -> Self {
        self.inner.retry_delay_secs = secs;
        self
    }

    /// Set a single proxy URL used for all protocols.
    pub fn proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.inner.proxy = Some(proxy_url.into());
        self
    }

    /// Set a per-protocol proxy override. The scheme is lowercased; `"http"`
    /// and `"https"` are routed to their respective protocols, any other key
    /// (e.g. `"all"`) applies to all protocols.
    pub fn protocol_proxy(
        mut self,
        scheme: impl Into<String>,
        proxy_url: impl Into<String>,
    ) -> Self {
        self.inner
            .proxies
            .insert(scheme.into().to_lowercase(), proxy_url.into());
        self
    }

    /// Set the list of proxies to rotate through. When non-empty, requests are
    /// distributed round-robin across one HTTP client per proxy.
    pub fn rotating_proxies<I, S>(mut self, proxies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.proxy_list = proxies.into_iter().map(Into::into).collect();
        self
    }

    /// Add a per-header override.  Key is lowercased automatically.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.headers.insert(key.into().to_lowercase(), value.into());
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.inner.user_agent = Some(ua.into());
        self
    }

    /// Enable or disable stealth header generation.
    pub fn stealth(mut self, enabled: bool) -> Self {
        self.inner.stealthy_headers = enabled;
        self
    }

    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.inner.follow_redirects = follow;
        self
    }

    pub fn verify_ssl(mut self, verify: bool) -> Self {
        self.inner.verify_ssl = verify;
        self
    }

    pub fn build(self) -> FetcherConfig {
        self.inner
    }
}
