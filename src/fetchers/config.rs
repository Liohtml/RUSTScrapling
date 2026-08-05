//! [`FetcherConfig`] and its builder: everything tunable about how a
//! [`Fetcher`](crate::fetchers::client::Fetcher) makes requests.

use crate::fetchers::constants;
use std::collections::HashMap;

/// Configuration for HTTP fetchers.
///
/// Build one with [`FetcherConfig::builder`] or start from
/// [`FetcherConfig::default`] (30s timeout, 3 retries with a 1s delay,
/// redirects followed up to 10 hops, SSL verified, stealth headers on,
/// 50 MiB body cap, no proxies).
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    /// Total per-request timeout in seconds (connect through body read).
    pub timeout_secs: u64,
    /// How many times a failed request is retried (in addition to the
    /// initial attempt).
    pub retries: u32,
    /// Fixed delay in seconds between retry attempts.
    pub retry_delay_secs: u64,
    /// Whether HTTP redirects are followed at all.
    pub follow_redirects: bool,
    /// Maximum number of redirect hops when `follow_redirects` is on.
    pub max_redirects: u32,
    /// Whether TLS certificates are verified. Disabling this is insecure
    /// and logs a warning when the fetcher is built.
    pub verify_ssl: bool,
    /// A single proxy URL applied to all protocols, used as a fallback
    /// after the per-protocol `proxies` map.
    pub proxy: Option<String>,
    /// Per-protocol proxy map: keys `"http"` / `"https"` bind a proxy to
    /// that scheme, any other key acts as a wildcard for all protocols.
    pub proxies: HashMap<String, String>,
    /// Proxy URLs to rotate through, one HTTP client is built per entry and
    /// selected round-robin per request. Takes precedence over `proxy` /
    /// `proxies` when non-empty.
    pub proxy_list: Vec<String>,
    /// Extra request headers (lowercase names). These win over the
    /// generated stealth headers.
    pub headers: HashMap<String, String>,
    /// Whether browser-like stealth headers are generated for each request
    /// (see [`FetcherConfig::build_headers`]).
    pub stealthy_headers: bool,
    /// Fixed User-Agent. When `None`, a user agent is picked
    /// deterministically per URL from a small pool of real browser strings.
    pub user_agent: Option<String>,
    /// Maximum response body size in bytes. Responses larger than this are
    /// aborted to protect against OOM. Defaults to 50 MiB.
    pub max_body_bytes: usize,
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
            max_body_bytes: 50 * 1024 * 1024,
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
        // Client-Hints (`sec-ch-ua*`) are a Chromium-only feature; sending them
        // alongside a Firefox User-Agent is a contradictory, easily-detected
        // fingerprint. Decide before moving `ua` into the header map.
        let is_chromium = ua.contains("Chrome") || ua.contains("Chromium");
        let platform = if ua.contains("Windows") {
            "\"Windows\""
        } else if ua.contains("Macintosh") || ua.contains("Mac OS") {
            "\"macOS\""
        } else if ua.contains("Linux") || ua.contains("X11") {
            "\"Linux\""
        } else {
            "\"Windows\""
        };
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

            // Client-Hints headers only for Chromium UAs, with a platform
            // derived from the UA (not hardcoded to Windows).
            if is_chromium {
                headers.entry("sec-ch-ua".to_string()).or_insert_with(|| {
                    "\"Google Chrome\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\""
                        .to_string()
                });

                headers
                    .entry("sec-ch-ua-mobile".to_string())
                    .or_insert_with(|| "?0".to_string());

                headers
                    .entry("sec-ch-ua-platform".to_string())
                    .or_insert_with(|| platform.to_string());
            }

            // Fetch-Metadata headers (`sec-fetch-*`) are sent by both Chromium
            // and Firefox, so they remain unconditional in stealth mode.
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

/// Builder for [`FetcherConfig`], starting from the default values.
#[derive(Debug, Default)]
#[must_use = "builders do nothing until `.build()` is called"]
pub struct FetcherConfigBuilder {
    inner: FetcherConfig,
}

impl FetcherConfigBuilder {
    /// Create a builder initialised with [`FetcherConfig::default`] values.
    pub fn new() -> Self {
        Self {
            inner: FetcherConfig::default(),
        }
    }

    /// Set the total per-request timeout, in seconds.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.inner.timeout_secs = secs;
        self
    }

    /// Set how many times a failed request is retried.
    pub fn retries(mut self, retries: u32) -> Self {
        self.inner.retries = retries;
        self
    }

    /// Set the fixed delay between retry attempts, in seconds.
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
        self.inner
            .headers
            .insert(key.into().to_lowercase(), value.into());
        self
    }

    /// Pin a fixed User-Agent string instead of the per-URL pick from the
    /// built-in browser pool.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.inner.user_agent = Some(ua.into());
        self
    }

    /// Enable or disable stealth header generation (on by default).
    pub fn stealth(mut self, enabled: bool) -> Self {
        self.inner.stealthy_headers = enabled;
        self
    }

    /// Enable or disable following HTTP redirects (on by default, up to
    /// 10 hops).
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.inner.follow_redirects = follow;
        self
    }

    /// Enable or disable TLS certificate verification. Disabling is
    /// insecure and logs a warning when the fetcher is built.
    pub fn verify_ssl(mut self, verify: bool) -> Self {
        self.inner.verify_ssl = verify;
        self
    }

    /// Cap on response body size in bytes. Responses larger than this are
    /// aborted before reading them fully into memory.
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.inner.max_body_bytes = bytes;
        self
    }

    /// Finish the builder and return the configured [`FetcherConfig`].
    #[must_use]
    pub fn build(self) -> FetcherConfig {
        self.inner
    }
}
