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
///
/// # Limitations
///
/// Feeds are parsed with the HTML parser, not a real XML parser:
///
/// - **XML namespaces** are not understood. Namespaced child tags show up
///   literally as keys (`<g:price>` → `"g:price"`), but a namespaced
///   `iter_tag` such as `"media:content"` cannot be used — the `:` is
///   parsed as a CSS pseudo-class and the selector matches nothing.
/// - **CDATA sections** (`<![CDATA[...]]>`) are not supported and their
///   content may be truncated or mangled; feeds that wrap descriptions in
///   CDATA will lose markup-heavy text.
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
    /// Case-insensitive, whole-tag-name matches only. Self-closing forms
    /// (`<link .../>`) are expanded to an explicit end tag because html5ever
    /// ignores the self-closing flag on unknown elements — the rewritten tag
    /// would stay open and swallow every following sibling.
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
                    // XML `S` after a tag name: space, tab, CR, LF.
                    let boundary_ok = matches!(
                        next,
                        Some(b'>')
                            | Some(b' ')
                            | Some(b'\t')
                            | Some(b'\r')
                            | Some(b'\n')
                            | Some(b'/')
                    ) || (closing && next.is_none());
                    if name_matches && boundary_ok {
                        if closing {
                            out.push_str("</");
                            out.push_str(to);
                            i = end;
                            continue 'outer;
                        }
                        // Find the unquoted `>` ending this tag; XML
                        // attribute values may legally contain `>`.
                        let mut j = end;
                        let mut quote: Option<u8> = None;
                        let gt = loop {
                            match bytes.get(j) {
                                None => break None,
                                Some(&b) => match quote {
                                    Some(q) if b == q => quote = None,
                                    Some(_) => {}
                                    None => match b {
                                        b'"' | b'\'' => quote = Some(b),
                                        b'>' => break Some(j),
                                        _ => {}
                                    },
                                },
                            }
                            j += 1;
                        };
                        out.push('<');
                        out.push_str(to);
                        match gt {
                            Some(gt) if bytes[gt - 1] == b'/' => {
                                // Self-closing: emit an explicit end tag.
                                out.push_str(&body[end..gt - 1]);
                                out.push_str("></");
                                out.push_str(to);
                                out.push('>');
                                i = gt + 1;
                            }
                            Some(gt) => {
                                out.push_str(&body[end..=gt]);
                                i = gt + 1;
                            }
                            // Truncated tag at EOF: copy the rest verbatim.
                            None => i = end,
                        }
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

#[cfg(test)]
mod tests {
    use super::XmlFeedSpider;
    use crate::parser::Selector;

    #[test]
    fn self_closing_link_does_not_swallow_siblings() {
        // html5ever ignores the self-closing flag on unknown elements; the
        // rewriter must emit an explicit end tag or `id`/`updated` would
        // become children of `xmlfeed-link` and vanish from the item.
        let body = r#"<feed><entry><title>A1</title><link href="https://a/1"/><id>urn:1</id><updated>2020</updated></entry></feed>"#;
        let rewritten = XmlFeedSpider::rewrite_void_tags(body);
        assert!(rewritten.contains(r#"<xmlfeed-link href="https://a/1"></xmlfeed-link>"#));
        let selector = Selector::from_html(&rewritten);
        let entries = selector.css("entry");
        let item = XmlFeedSpider::node_to_item(&entries[0]);
        let obj = item.as_object().unwrap();
        assert_eq!(obj.get("title").unwrap(), "A1");
        assert_eq!(obj.get("id").unwrap(), "urn:1");
        assert_eq!(obj.get("updated").unwrap(), "2020");
        assert_eq!(obj.get("link").unwrap(), "");
    }

    #[test]
    fn newline_after_tag_name_is_a_boundary() {
        // Pretty-printers wrap attributes: `<link\n>` is valid XML `S`.
        let rewritten = XmlFeedSpider::rewrite_void_tags("<link\n>https://a/1</link>");
        assert_eq!(rewritten, "<xmlfeed-link\n>https://a/1</xmlfeed-link>");
        let rewritten = XmlFeedSpider::rewrite_void_tags("<link\r\n href=\"x\">t</link>");
        assert_eq!(rewritten, "<xmlfeed-link\r\n href=\"x\">t</xmlfeed-link>");
    }

    #[test]
    fn gt_inside_quoted_attribute_does_not_end_the_tag() {
        // `>` is legal inside XML attribute values.
        let rewritten = XmlFeedSpider::rewrite_void_tags(r#"<link title="a>b"/><id>1</id>"#);
        assert_eq!(
            rewritten,
            r#"<xmlfeed-link title="a>b"></xmlfeed-link><id>1</id>"#
        );
    }

    #[test]
    fn non_matching_and_truncated_tags_pass_through() {
        assert_eq!(
            XmlFeedSpider::rewrite_void_tags("<linkage>x</linkage>"),
            "<linkage>x</linkage>"
        );
        // Truncated tag at EOF: copied verbatim after the rewritten name.
        assert_eq!(
            XmlFeedSpider::rewrite_void_tags("<link href=\"x"),
            "<xmlfeed-link href=\"x"
        );
        assert_eq!(XmlFeedSpider::rewrite_void_tags("</link"), "</xmlfeed-link");
    }
}
