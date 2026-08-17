//! `XmlFeedSpider`, ported from upstream Scrapling's `XMLFeedSpider`
//! (v0.4.13): iterate over the nodes of an XML feed (RSS, Atom, product
//! feeds) and turn each into items.
//!
//! Gzip-compressed feed files (`.xml.gz`) are decompressed transparently
//! by the fetcher (magic-byte detection in the body decoder), in addition
//! to transport-level `Content-Encoding: gzip`.

use crate::parser::Selector;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

/// Callback turning one feed node (e.g. one RSS `<item>`) into items.
/// Receives the full response plus the node's [`Selector`].
pub type ParseNodeFn =
    Arc<dyn Fn(&SpiderResponse, &Selector) -> Vec<serde_json::Value> + Send + Sync>;

/// A [`Spider`] that iterates over the nodes of XML feeds.
///
/// Every response is parsed and each element matching `iter_tag` (default
/// `"item"`, the RSS entry tag — use `"entry"` for Atom) is handed to the
/// `parse_node` callback. Without a callback, each node is converted to an
/// object mapping its **child element names to their text content**
/// (`<title>X</title>` → `"title": "X"`); when a child tag repeats, the
/// last occurrence wins. Feeds are terminal: no follow-up requests are
/// generated.
pub struct XmlFeedSpider {
    name: String,
    feed_urls: Vec<String>,
    iter_tag: String,
    parse_node: Option<ParseNodeFn>,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    development_mode: bool,
    robots_txt_obey: bool,
}

/// XML tags that the HTML parser treats as void elements, swallowing their
/// text content — `<link>` is THE critical one (every RSS item's URL).
/// They are rewritten to `xmlfeed-*` before parsing and translated back
/// when item keys are emitted.
const VOID_TAG_REWRITES: &[(&str, &str)] = &[("link", "xmlfeed-link"), ("meta", "xmlfeed-meta")];

impl XmlFeedSpider {
    /// Start building an `XmlFeedSpider` with the given spider name.
    pub fn builder(name: &str) -> XmlFeedSpiderBuilder {
        XmlFeedSpiderBuilder::new(name)
    }

    /// Rewrite XML tags that HTML parsing would mangle (HTML void elements
    /// like `<link>` cannot have children, so their text would be lost).
    /// Case-insensitive, whole-tag-name matches only.
    fn rewrite_void_tags(body: &str) -> String {
        let mut out = String::with_capacity(body.len());
        let bytes = body.as_bytes();
        let mut i = 0;
        'outer: while i < bytes.len() {
            if bytes[i] == b'<' {
                let (start, closing) = if bytes.get(i + 1) == Some(&b'/') {
                    (i + 2, true)
                } else {
                    (i + 1, false)
                };
                for (from, to) in VOID_TAG_REWRITES {
                    let end = start + from.len();
                    let next = bytes.get(end);
                    let name_matches = body
                        .get(start..end)
                        .is_some_and(|s| s.eq_ignore_ascii_case(from));
                    let boundary_ok =
                        matches!(next, Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'/'))
                            || (closing && next.is_none());
                    if name_matches && boundary_ok {
                        out.push('<');
                        if closing {
                            out.push('/');
                        }
                        out.push_str(to);
                        i = end;
                        continue 'outer;
                    }
                }
            }
            // Advance by whole characters so multibyte input stays intact.
            let ch_len = body[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&body[i..i + ch_len]);
            i += ch_len;
        }
        out
    }

    /// Translate a possibly-rewritten tag name back to its original.
    fn original_tag(tag: &str) -> &str {
        VOID_TAG_REWRITES
            .iter()
            .find(|(_, to)| *to == tag)
            .map_or(tag, |(from, _)| from)
    }

    /// Default node conversion: child element names → recursive text.
    fn node_to_item(node: &Selector) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for child in &node.children() {
            let tag = child.tag().to_string();
            if tag.starts_with('#') {
                continue; // text/comment placeholder tags
            }
            let text = child.get_all_text("", false, &[], None);
            obj.insert(
                Self::original_tag(&tag).to_string(),
                serde_json::Value::String(text.as_str().trim().to_string()),
            );
        }
        serde_json::Value::Object(obj)
    }
}

#[async_trait]
impl Spider for XmlFeedSpider {
    fn name(&self) -> &str {
        &self.name
    }
    fn start_urls(&self) -> Vec<String> {
        self.feed_urls.clone()
    }
    fn allowed_domains(&self) -> HashSet<String> {
        self.allowed_domains.clone()
    }
    fn concurrent_requests(&self) -> u32 {
        self.concurrent_requests
    }
    fn development_mode(&self) -> bool {
        self.development_mode
    }
    fn robots_txt_obey(&self) -> bool {
        self.robots_txt_obey
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        // Parse a rewritten copy of the body so HTML-void feed tags
        // (notably <link>) keep their text; translate the user's iter_tag
        // too in case it names one of them. Custom parse_node callbacks see
        // the rewritten tree: address rewritten tags as `xmlfeed-link` /
        // `xmlfeed-meta` (documented on the builder).
        let rewritten = Self::rewrite_void_tags(response.text());
        let selector = Selector::from_html_with_url(&rewritten, response.url());
        let iter_tag = VOID_TAG_REWRITES
            .iter()
            .find(|(from, _)| from.eq_ignore_ascii_case(&self.iter_tag))
            .map_or(self.iter_tag.clone(), |(_, to)| (*to).to_string());
        let mut items = Vec::new();
        for node in &selector.css(&iter_tag) {
            match &self.parse_node {
                Some(f) => items.extend(f(&response, node)),
                None => items.push(Self::node_to_item(node)),
            }
        }
        (items, vec![])
    }
}

/// Builder for [`XmlFeedSpider`].
#[must_use = "a builder does nothing until `.build()` is called"]
pub struct XmlFeedSpiderBuilder {
    spider: XmlFeedSpider,
}

impl XmlFeedSpiderBuilder {
    fn new(name: &str) -> Self {
        Self {
            spider: XmlFeedSpider {
                name: name.to_string(),
                feed_urls: Vec::new(),
                iter_tag: "item".to_string(),
                parse_node: None,
                allowed_domains: HashSet::new(),
                concurrent_requests: 4,
                development_mode: false,
                robots_txt_obey: false,
            },
        }
    }

    /// Add a feed URL to fetch.
    pub fn feed_url(mut self, url: &str) -> Self {
        self.spider.feed_urls.push(url.to_string());
        self
    }

    /// Add several feed URLs.
    pub fn feed_urls(mut self, urls: impl IntoIterator<Item = String>) -> Self {
        self.spider.feed_urls.extend(urls);
        self
    }

    /// The element to iterate over (default `"item"`; use `"entry"` for
    /// Atom feeds). Any CSS selector works.
    pub fn iter_tag(mut self, tag: &str) -> Self {
        self.spider.iter_tag = tag.to_string();
        self
    }

    /// Callback turning one feed node into items; overrides the default
    /// child-elements-to-object conversion. Note: the parsed tree has
    /// HTML-void feed tags rewritten (`<link>` → `<xmlfeed-link>`,
    /// `<meta>` → `<xmlfeed-meta>`) so their text survives HTML parsing —
    /// address them by the rewritten name in CSS queries.
    pub fn parse_node(mut self, f: ParseNodeFn) -> Self {
        self.spider.parse_node = Some(f);
        self
    }

    /// Restrict the crawl to these domains.
    pub fn allowed_domains(mut self, domains: impl IntoIterator<Item = String>) -> Self {
        self.spider.allowed_domains.extend(domains);
        self
    }

    /// Global concurrency limit (default 4).
    pub fn concurrent_requests(mut self, n: u32) -> Self {
        self.spider.concurrent_requests = n;
        self
    }

    /// Cache responses to disk for development iteration.
    pub fn development_mode(mut self, on: bool) -> Self {
        self.spider.development_mode = on;
        self
    }

    /// Respect robots.txt.
    pub fn robots_txt_obey(mut self, on: bool) -> Self {
        self.spider.robots_txt_obey = on;
        self
    }

    /// Finish building the spider.
    pub fn build(self) -> XmlFeedSpider {
        self.spider
    }
}
