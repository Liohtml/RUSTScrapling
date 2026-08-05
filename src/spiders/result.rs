//! Crawl outputs: the scraped [`ItemList`] with CSV/JSON/JSONL/XML export,
//! live [`CrawlStats`], and the [`CrawlResult`] returned by
//! [`CrawlerEngine::crawl`](crate::spiders::engine::CrawlerEngine::crawl).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

/// A list of scraped items (JSON values), exportable to JSON, JSON Lines,
/// CSV, and XML.
#[derive(Debug, Clone, Default)]
pub struct ItemList {
    items: Vec<serde_json::Value>,
}

impl ItemList {
    /// Create an empty list.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Append one item.
    pub fn push(&mut self, item: serde_json::Value) {
        self.items.push(item);
    }

    /// Number of items in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Write the items as one JSON array to a file — pretty-printed when
    /// `indent > 0`, compact otherwise (the exact indent width is not
    /// configurable).
    pub fn to_json(&self, path: &Path, indent: usize) -> io::Result<()> {
        let buf = if indent > 0 {
            serde_json::to_vec_pretty(&self.items).map_err(io::Error::other)?
        } else {
            serde_json::to_vec(&self.items).map_err(io::Error::other)?
        };
        fs::write(path, buf)
    }

    /// Write the items as JSON Lines (one compact JSON value per line) to
    /// a file.
    pub fn to_jsonl(&self, path: &Path) -> io::Result<()> {
        let mut file = fs::File::create(path)?;
        for item in &self.items {
            let line = serde_json::to_string(item).map_err(io::Error::other)?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    /// Write items as CSV (RFC 4180 quoting). The header is the union of all
    /// keys across all items in first-seen order, so heterogeneous item
    /// shapes export without data loss — an item simply leaves cells empty
    /// for keys it doesn't have. Nested values (objects/arrays) are
    /// serialized as JSON strings; null becomes an empty cell. Items that
    /// are not JSON objects are skipped.
    ///
    /// Values are exported verbatim (RFC 4180 only). If the file will be
    /// opened in a spreadsheet application, be aware that scraped cell
    /// values starting with `=`, `+`, `-`, or `@` may be interpreted as
    /// formulas there — sanitize downstream if that matters for your use.
    pub fn to_csv(&self, path: &Path) -> io::Result<()> {
        let columns = self.union_of_keys();
        let mut out = String::new();

        let write_row = |fields: &mut dyn Iterator<Item = String>, out: &mut String| {
            let mut first = true;
            for field in fields {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&csv_escape(&field));
            }
            out.push_str("\r\n");
        };

        write_row(&mut columns.iter().cloned(), &mut out);
        for item in &self.items {
            let obj = match item.as_object() {
                Some(o) => o,
                None => continue,
            };
            write_row(
                &mut columns
                    .iter()
                    .map(|key| obj.get(key).map(flatten_value).unwrap_or_default()),
                &mut out,
            );
        }
        fs::write(path, out)
    }

    /// Write items as XML: `<items><item><key>value</key>…</item></items>`.
    /// Keys are sanitized into valid XML element names (invalid characters
    /// become `_`; a leading digit is prefixed); text content is entity
    /// escaped. Nested values (objects/arrays) are serialized as JSON
    /// strings; null becomes an empty element. Items that are not JSON
    /// objects are skipped.
    pub fn to_xml(&self, path: &Path) -> io::Result<()> {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<items>\n");
        for item in &self.items {
            let obj = match item.as_object() {
                Some(o) => o,
                None => continue,
            };
            out.push_str("  <item>\n");
            for (key, value) in obj {
                let tag = xml_element_name(key);
                out.push_str("    <");
                out.push_str(&tag);
                out.push('>');
                out.push_str(&xml_escape(&flatten_value(value)));
                out.push_str("</");
                out.push_str(&tag);
                out.push_str(">\n");
            }
            out.push_str("  </item>\n");
        }
        out.push_str("</items>\n");
        fs::write(path, out)
    }

    /// Union of all object keys across all items, in first-seen order.
    fn union_of_keys(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut columns = Vec::new();
        for item in &self.items {
            if let Some(obj) = item.as_object() {
                for key in obj.keys() {
                    if seen.insert(key.clone()) {
                        columns.push(key.clone());
                    }
                }
            }
        }
        columns
    }
}

/// A JSON value as a flat cell/text value: strings verbatim, scalars via
/// display, null empty, and nested objects/arrays as compact JSON strings
/// (so heterogeneous nesting survives flat formats losslessly).
fn flatten_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        nested => nested.to_string(),
    }
}

/// RFC 4180: quote a field if it contains a comma, quote, or line break;
/// double any quotes inside.
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn xml_escape(text: &str) -> String {
    // XML 1.0 forbids most C0 control characters (and U+FFFE/U+FFFF) even
    // as character references — scraped content routinely contains them,
    // and a single stray byte would make the whole export unparseable.
    // Drop them; escape the five markup characters.
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            '\u{FFFE}' | '\u{FFFF}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Sanitize an arbitrary JSON key into a valid XML element name: characters
/// outside [A-Za-z0-9_.-] become `_`, and a name that would start with a
/// digit, dot, or dash is prefixed with `_`. Empty keys become `_`.
fn xml_element_name(key: &str) -> String {
    let mut name: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        name.push('_');
    }
    let first = name.chars().next().expect("non-empty");
    if !(first.is_ascii_alphabetic() || first == '_') {
        name.insert(0, '_');
    }
    name
}

impl IntoIterator for ItemList {
    type Item = serde_json::Value;
    type IntoIter = std::vec::IntoIter<serde_json::Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Statistics gathered during a crawl, updated live by the engine and
/// returned in the final [`CrawlResult`].
#[derive(Debug, Clone)]
pub struct CrawlStats {
    /// Live (non-cache) responses received, regardless of status.
    pub requests_count: u64,
    /// The spider's configured global concurrency limit (copied at crawl
    /// start, informational).
    pub concurrent_requests: u32,
    /// The spider's configured per-domain concurrency limit (0 = none).
    pub concurrent_requests_per_domain: u32,
    /// Requests that failed at the network/session level (after the
    /// fetcher's own retries).
    pub failed_requests_count: u64,
    /// Requests dropped by the `allowed_domains` filter (including URLs
    /// with no parseable host).
    pub offsite_requests_count: u64,
    /// Requests dropped because robots.txt disallowed them.
    pub robots_disallowed_count: u64,
    /// Requests served from the development-mode cache.
    pub cache_hits: u64,
    /// Requests that checked the dev cache and found no usable entry.
    pub cache_misses: u64,
    /// Total decoded response bytes across all live responses.
    pub response_bytes: u64,
    /// Items kept after the `on_scraped_item` pipeline.
    pub items_scraped: u64,
    /// Items dropped by the `on_scraped_item` pipeline (it returned
    /// `None`).
    pub items_dropped: u64,
    /// Responses classified as blocked (each may trigger a retry).
    pub blocked_requests_count: u64,
    /// The spider's configured download delay in seconds (informational).
    pub download_delay: f64,
    /// When the crawl started; set by the engine at the top of `crawl()`.
    pub start_time: Option<Instant>,
    /// When the crawl ended; `None` while it is still running.
    pub end_time: Option<Instant>,
    /// Count of live responses per HTTP status code.
    pub response_status_count: HashMap<u16, u64>,
    /// Decoded response bytes per domain, populated by the engine on every
    /// live (non-cache) response.
    pub domains_response_bytes: HashMap<String, u64>,
    /// Live requests per session id ("default" for requests without one),
    /// populated by the engine.
    pub sessions_requests_count: HashMap<String, u64>,
    /// Free-form slot for user pipelines to attach their own metrics to a
    /// crawl result; the engine itself never writes it.
    pub custom_stats: HashMap<String, serde_json::Value>,
}

impl Default for CrawlStats {
    fn default() -> Self {
        Self {
            requests_count: 0,
            concurrent_requests: 0,
            concurrent_requests_per_domain: 0,
            failed_requests_count: 0,
            offsite_requests_count: 0,
            robots_disallowed_count: 0,
            cache_hits: 0,
            cache_misses: 0,
            response_bytes: 0,
            items_scraped: 0,
            items_dropped: 0,
            blocked_requests_count: 0,
            download_delay: 0.0,
            start_time: None,
            end_time: None,
            response_status_count: HashMap::new(),
            domains_response_bytes: HashMap::new(),
            sessions_requests_count: HashMap::new(),
            custom_stats: HashMap::new(),
        }
    }
}

impl CrawlStats {
    /// Bump the live-response counter by one.
    pub fn increment_requests_count(&mut self) {
        self.requests_count += 1;
    }

    /// Bump the per-status counter for `status` by one.
    pub fn increment_status(&mut self, status: u16) {
        *self.response_status_count.entry(status).or_insert(0) += 1;
    }

    /// Add `bytes` to the total decoded response bytes.
    pub fn increment_response_bytes(&mut self, bytes: u64) {
        self.response_bytes += bytes;
    }

    /// Wall-clock duration of the crawl in seconds: end minus start when
    /// finished, time since start while still running, `0.0` before start.
    #[must_use]
    pub fn elapsed_seconds(&self) -> f64 {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start).as_secs_f64(),
            (Some(start), None) => start.elapsed().as_secs_f64(),
            _ => 0.0,
        }
    }

    /// Average live requests per second over [`CrawlStats::elapsed_seconds`]
    /// (`0.0` when no time has elapsed).
    #[must_use]
    pub fn requests_per_second(&self) -> f64 {
        let elapsed = self.elapsed_seconds();
        if elapsed > 0.0 {
            self.requests_count as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Result of a crawl run, returned by
/// [`CrawlerEngine::crawl`](crate::spiders::engine::CrawlerEngine::crawl).
#[derive(Debug)]
pub struct CrawlResult {
    /// Statistics gathered during the crawl.
    pub stats: CrawlStats,
    /// All items scraped (and kept by the item pipeline).
    pub items: ItemList,
    /// `true` when the crawl was paused (a checkpoint was written) rather
    /// than run to completion.
    pub paused: bool,
}

impl CrawlResult {
    /// Whether the crawl ran to completion (the opposite of `paused`).
    #[must_use]
    pub fn completed(&self) -> bool {
        !self.paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_items() -> ItemList {
        let mut items = ItemList::new();
        items.push(json!({
            "title": "Plain",
            "price": 9.99,
            "tags": ["a", "b"],
        }));
        items.push(json!({
            "title": "Quote \"and\", comma",
            "sku": "X-1",
            "meta": {"color": "red"},
        }));
        items
    }

    #[test]
    fn csv_unions_heterogeneous_keys_and_escapes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("items.csv");
        sample_items().to_csv(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Header: union of keys in first-seen order.
        assert_eq!(lines[0], "title,price,tags,sku,meta");
        // Item 1: no sku/meta -> empty trailing cells; array as JSON string.
        assert_eq!(lines[1], "Plain,9.99,\"[\"\"a\"\",\"\"b\"\"]\",,");
        // Item 2: quotes doubled, field quoted; no price/tags -> empty cells.
        assert_eq!(
            lines[2],
            "\"Quote \"\"and\"\", comma\",,,X-1,\"{\"\"color\"\":\"\"red\"\"}\""
        );
    }

    #[test]
    fn csv_skips_non_object_items() {
        let mut items = ItemList::new();
        items.push(json!("just a string"));
        items.push(json!({"k": "v"}));
        let dir = tempdir().unwrap();
        let path = dir.path().join("items.csv");
        items.to_csv(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 2, "header + one object row");
    }

    #[test]
    fn xml_escapes_text_and_sanitizes_element_names() {
        let mut items = ItemList::new();
        items.push(json!({
            "title": "a < b & c > 'd'",
            "weird key!": "v",
            "1st": "leading digit",
            "": "empty key",
        }));
        let dir = tempdir().unwrap();
        let path = dir.path().join("items.xml");
        items.to_xml(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(content.contains("<title>a &lt; b &amp; c &gt; &apos;d&apos;</title>"));
        assert!(content.contains("<weird_key_>v</weird_key_>"));
        assert!(content.contains("<_1st>leading digit</_1st>"));
        assert!(content.contains("<_>empty key</_>"));
        assert!(content.trim_end().ends_with("</items>"));
    }

    #[test]
    fn xml_strips_invalid_control_characters() {
        // XML 1.0 forbids most C0 controls even as entities; a single stray
        // byte from scraped content must not make the export unparseable.
        let mut items = ItemList::new();
        items.push(json!({
            "text": "bell\u{0007} null\u{0000} keep\ttab\nnewline \u{FFFF}end",
        }));
        let dir = tempdir().unwrap();
        let path = dir.path().join("items.xml");
        items.to_xml(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<text>bell null keep\ttab\nnewline end</text>"));
        assert!(!content.contains('\u{0007}'));
        assert!(!content.contains('\u{FFFF}'));
    }

    #[test]
    fn csv_rows_end_with_crlf() {
        // RFC 4180 rows terminate with CRLF; assert at the byte level since
        // lines() would silently strip the \r.
        let mut items = ItemList::new();
        items.push(json!({"k": "v"}));
        let dir = tempdir().unwrap();
        let path = dir.path().join("crlf.csv");
        items.to_csv(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, b"k\r\nv\r\n");
    }

    #[test]
    fn xml_nested_values_become_json_strings() {
        let mut items = ItemList::new();
        items.push(json!({"meta": {"a": 1}, "none": null}));
        let dir = tempdir().unwrap();
        let path = dir.path().join("items.xml");
        items.to_xml(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<meta>{&quot;a&quot;:1}</meta>"));
        assert!(content.contains("<none></none>"));
    }

    #[test]
    fn empty_item_list_exports_header_only_csv_and_empty_xml() {
        let items = ItemList::new();
        let dir = tempdir().unwrap();
        let csv = dir.path().join("empty.csv");
        let xml = dir.path().join("empty.xml");
        items.to_csv(&csv).unwrap();
        items.to_xml(&xml).unwrap();
        assert_eq!(std::fs::read_to_string(&csv).unwrap(), "\r\n");
        let xml_content = std::fs::read_to_string(&xml).unwrap();
        assert!(xml_content.contains("<items>\n</items>"));
    }
}
