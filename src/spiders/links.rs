//! Pure URL discovery primitive, ported from upstream Scrapling's
//! `spiders/links.py` (v0.4.8).

use crate::spiders::response::SpiderResponse;
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;
use url::Url;

/// URL schemes a [`LinkExtractor`] will keep.
const VALID_SCHEMES: [&str; 3] = ["http", "https", "file"];

/// File extensions dropped by default, matching upstream Scrapling's
/// `IGNORED_EXTENSIONS`.
pub const IGNORED_EXTENSIONS: [&str; 91] = [
    // archives
    "7z", "7zip", "bz2", "rar", "tar", "tar.gz", "xz", "zip", // images
    "mng", "pct", "bmp", "gif", "jpg", "jpeg", "png", "pst", "psp", "tif", "tiff", "ai", "drw",
    "dxf", "eps", "ps", "svg", "cdr", "ico", "webp", // audio
    "mp3", "wma", "ogg", "wav", "ra", "aac", "mid", "au", "aiff", // video
    "3gp", "asf", "asx", "avi", "mov", "mp4", "mpg", "qt", "rm", "swf", "wmv", "m4a", "m4v", "flv",
    "webm", // office suites
    "xls", "xlsm", "xlsx", "xltm", "xltx", "potm", "potx", "ppt", "pptm", "pptx", "pps", "doc",
    "docb", "docm", "docx", "dotm", "dotx", "odt", "ods", "odg", "odp", // other
    "css", "pdf", "exe", "bin", "rss", "dmg", "iso", "apk", "jar", "sh", "rb", "js", "hta", "bat",
    "cpl", "msi", "msp", "py",
];

/// Optional hook applied to each URL before filtering; return `None` to drop.
pub type ProcessFn = Arc<dyn Fn(String) -> Option<String> + Send + Sync>;

/// Extracts and filters URLs from a [`SpiderResponse`] (or a single URL via
/// [`LinkExtractor::matches`]).
///
/// All matching is regex-based. Domain filters match the exact host or any
/// subdomain (`example.com` matches `api.example.com`). `deny` takes
/// precedence over `allow`.
///
/// Configure it with the chained builder-style methods
/// ([`LinkExtractor::allow`], [`LinkExtractor::deny_domains`], …), then
/// call [`LinkExtractor::extract`] on responses (or
/// [`LinkExtractor::matches`] on single URLs).
#[derive(Clone)]
#[must_use = "a LinkExtractor does nothing until `.extract()` or `.matches()` is called"]
pub struct LinkExtractor {
    allow: Vec<Regex>,
    deny: Vec<Regex>,
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
    restrict_css: Vec<String>,
    tags: Vec<String>,
    attrs: Vec<String>,
    canonicalize: bool,
    strip: bool,
    keep_fragment: bool,
    deny_extensions: HashSet<String>,
    process: Option<ProcessFn>,
}

impl Default for LinkExtractor {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            deny: Vec::new(),
            allow_domains: Vec::new(),
            deny_domains: Vec::new(),
            restrict_css: Vec::new(),
            tags: vec!["a".to_string(), "area".to_string()],
            attrs: vec!["href".to_string()],
            canonicalize: true,
            strip: true,
            keep_fragment: false,
            deny_extensions: IGNORED_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            process: None,
        }
    }
}

impl std::fmt::Debug for LinkExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkExtractor")
            .field(
                "allow",
                &self.allow.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
            )
            .field(
                "deny",
                &self.deny.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
            )
            .field("allow_domains", &self.allow_domains)
            .field("deny_domains", &self.deny_domains)
            .field("restrict_css", &self.restrict_css)
            .field("tags", &self.tags)
            .field("attrs", &self.attrs)
            .field("canonicalize", &self.canonicalize)
            .field("strip", &self.strip)
            .field("keep_fragment", &self.keep_fragment)
            .finish_non_exhaustive()
    }
}

impl LinkExtractor {
    /// Create an extractor with the default configuration: `a`/`area`
    /// tags' `href` attributes, canonicalization and whitespace-stripping
    /// on, and the standard extension deny list
    /// ([`IGNORED_EXTENSIONS`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Regex pattern(s) URLs must match to be kept. Empty means match all.
    /// Invalid patterns are rejected with the regex error.
    pub fn allow<I, S>(mut self, patterns: I) -> Result<Self, regex::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for p in patterns {
            self.allow.push(Regex::new(p.as_ref())?);
        }
        Ok(self)
    }

    /// Regex pattern(s) URLs must NOT match. Takes precedence over `allow`.
    pub fn deny<I, S>(mut self, patterns: I) -> Result<Self, regex::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for p in patterns {
            self.deny.push(Regex::new(p.as_ref())?);
        }
        Ok(self)
    }

    /// Domain(s) to keep: the exact host or any subdomain.
    pub fn allow_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allow_domains
            .extend(domains.into_iter().map(|d| d.as_ref().to_lowercase()));
        self
    }

    /// Domain(s) to exclude. Same matching rules as `allow_domains`.
    pub fn deny_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.deny_domains
            .extend(domains.into_iter().map(|d| d.as_ref().to_lowercase()));
        self
    }

    /// CSS selectors to scope DOM extraction to. Empty means whole page.
    pub fn restrict_css<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.restrict_css
            .extend(selectors.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    /// Element tags to look for links in. Default `["a", "area"]`.
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.tags = tags.into_iter().map(|t| t.as_ref().to_string()).collect();
        self
    }

    /// Attributes on those tags to read URLs from. Default `["href"]`.
    pub fn attrs<I, S>(mut self, attrs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.attrs = attrs.into_iter().map(|a| a.as_ref().to_string()).collect();
        self
    }

    /// Canonicalize URLs (sort query params, drop fragment). Default `true`.
    pub fn canonicalize(mut self, on: bool) -> Self {
        self.canonicalize = on;
        self
    }

    /// Strip whitespace from extracted URLs. Default `true`.
    pub fn strip(mut self, on: bool) -> Self {
        self.strip = on;
        self
    }

    /// Preserve the URL fragment when canonicalizing. Default `false`.
    pub fn keep_fragment(mut self, on: bool) -> Self {
        self.keep_fragment = on;
        self
    }

    /// Replace the extension deny list (default [`IGNORED_EXTENSIONS`]).
    /// Leading dots are stripped, comparison is case-insensitive.
    pub fn deny_extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.deny_extensions = extensions
            .into_iter()
            .map(|e| e.as_ref().trim_start_matches('.').to_lowercase())
            .collect();
        self
    }

    /// Hook applied to each absolute URL before filtering; return `None` to
    /// drop the URL.
    pub fn process(mut self, f: ProcessFn) -> Self {
        self.process = Some(f);
        self
    }

    /// Return absolute, filtered, deduped URLs from `response`, preserving
    /// document order.
    ///
    /// Pipeline per candidate: read the configured attributes off the
    /// configured tags (within `restrict_css` scopes if any), strip
    /// whitespace, absolutize against the response URL, run the `process`
    /// hook, canonicalize, then apply the scheme / extension / allow /
    /// deny / domain filters and dedup.
    #[must_use]
    pub fn extract(&self, response: &SpiderResponse) -> Vec<String> {
        let root = response.selector();
        let scopes = if self.restrict_css.is_empty() {
            vec![root]
        } else {
            let mut scopes = Vec::new();
            for css in &self.restrict_css {
                for el in root.css(css) {
                    scopes.push(el);
                }
            }
            scopes
        };

        let base = Url::parse(response.url()).ok();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        // One selector-list query per scope ("a, area") so results come back
        // in true document order rather than grouped by tag.
        let tag_selector = self.tags.join(", ");
        for scope in &scopes {
            for el in scope.css(&tag_selector) {
                for attr in &self.attrs {
                    let Some(value) = el.get_attribute(attr) else {
                        continue;
                    };
                    let mut candidate = value.to_string();
                    if self.strip {
                        candidate = candidate.trim().to_string();
                        if candidate.is_empty() {
                            continue;
                        }
                    }
                    let absolute = match &base {
                        Some(b) => match b.join(&candidate) {
                            Ok(u) => u.to_string(),
                            Err(_) => {
                                log::debug!("Skipping the extraction of bad URL {candidate:?}");
                                continue;
                            }
                        },
                        None => candidate,
                    };
                    let Some(processed) = self
                        .process
                        .as_ref()
                        .map_or(Some(absolute.clone()), |f| f(absolute))
                    else {
                        continue;
                    };
                    let finalized = if self.canonicalize {
                        canonicalize_url(&processed, self.keep_fragment)
                    } else {
                        processed
                    };
                    if !self.url_passes(&finalized) {
                        continue;
                    }
                    if seen.insert(finalized.clone()) {
                        out.push(finalized);
                    }
                }
            }
        }
        out
    }

    /// URL-only filter (no response extraction). Applies
    /// allow/deny/domain/extension rules to a single URL. Used by
    /// `SitemapSpider` to dispatch sitemap URLs through `CrawlRule`s without
    /// needing a response.
    #[must_use]
    pub fn matches(&self, url: &str) -> bool {
        let url = if self.canonicalize {
            canonicalize_url(url, self.keep_fragment)
        } else {
            url.to_string()
        };
        self.url_passes(&url)
    }

    fn url_passes(&self, url: &str) -> bool {
        let scheme = url.split("://").next().unwrap_or("");
        if !VALID_SCHEMES.contains(&scheme) {
            return false;
        }

        if !self.deny_extensions.is_empty()
            && extension_chains(url)
                .iter()
                .any(|ext| self.deny_extensions.contains(ext))
        {
            return false;
        }

        if !self.allow.is_empty() && !self.allow.iter().any(|p| p.is_match(url)) {
            return false;
        }
        if self.deny.iter().any(|p| p.is_match(url)) {
            return false;
        }

        if !self.allow_domains.is_empty() || !self.deny_domains.is_empty() {
            let host = Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                .unwrap_or_default();
            if !self.allow_domains.is_empty()
                && !self
                    .allow_domains
                    .iter()
                    .any(|d| host == *d || host.ends_with(&format!(".{d}")))
            {
                return false;
            }
            if self
                .deny_domains
                .iter()
                .any(|d| host == *d || host.ends_with(&format!(".{d}")))
            {
                return false;
            }
        }
        true
    }
}

/// All extension suffix chains of the URL's last path segment, longest
/// first: `/archive.tar.gz` yields `["tar.gz", "gz"]`. Compound extensions
/// are included so a deny list containing `tar.gz` matches (upstream fix
/// `0d35a3f`).
fn extension_chains(url: &str) -> Vec<String> {
    let path = match Url::parse(url) {
        Ok(u) => u.path().to_string(),
        Err(_) => url.split(['?', '#']).next().unwrap_or("").to_string(),
    };
    let last = path.rsplit('/').next().unwrap_or("").to_lowercase();
    if !last.contains('.') {
        return Vec::new();
    }
    let parts: Vec<&str> = last.split('.').collect();
    (1..parts.len())
        .filter(|&i| !parts[i].is_empty())
        .map(|i| parts[i..].join("."))
        .collect()
}

/// Canonicalize a URL: sort query parameters and drop the fragment unless
/// `keep_fragment`. Unparseable URLs are returned unchanged.
///
/// Query parameters are sorted as *raw* `&`-separated segments (empty
/// segments dropped) rather than decoded key/value pairs, so percent-encoded
/// bytes that are not valid UTF-8 (e.g. `%FF`) survive canonicalization
/// instead of being mangled through a decode/re-encode round trip. An empty
/// query (`/p?`) is normalized away so it dedupes with `/p`.
#[must_use]
pub fn canonicalize_url(url: &str, keep_fragment: bool) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    if let Some(query) = parsed.query() {
        let mut segments: Vec<&str> = query.split('&').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            parsed.set_query(None);
        } else {
            segments.sort_unstable();
            let sorted = segments.join("&");
            parsed.set_query(Some(&sorted));
        }
    }
    if !keep_fragment {
        parsed.set_fragment(None);
    }
    parsed.to_string()
}
