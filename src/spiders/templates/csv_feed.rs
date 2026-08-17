//! `CsvFeedSpider`, ported from upstream Scrapling's `CSVFeedSpider`
//! (v0.4.13): iterate over the rows of CSV feeds as keyed objects.
//!
//! Gzip-compressed feed files (`.csv.gz`) are decompressed transparently
//! by the fetcher (magic-byte detection in the body decoder), in addition
//! to transport-level `Content-Encoding: gzip`.

use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

/// Callback turning one CSV row (header → cell map, in column order) into
/// items.
pub type ParseRowFn = Arc<
    dyn Fn(&SpiderResponse, &serde_json::Map<String, serde_json::Value>) -> Vec<serde_json::Value>
        + Send
        + Sync,
>;

/// A [`Spider`] that iterates over CSV feed rows.
///
/// Each response body is parsed as RFC 4180 CSV (configurable delimiter,
/// `"`-quoted fields with `""` escapes, LF or CRLF row endings). The first
/// row provides the column names unless `headers` overrides them (in which
/// case the first row is data). Every row becomes a header → cell object
/// handed to `parse_row`; without a callback the row object itself is the
/// item. Rows shorter than the header get empty strings for the missing
/// columns; extra cells beyond the header are dropped. Feeds are terminal:
/// no follow-up requests are generated.
pub struct CsvFeedSpider {
    name: String,
    feed_urls: Vec<String>,
    delimiter: char,
    headers: Option<Vec<String>>,
    parse_row: Option<ParseRowFn>,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    development_mode: bool,
    robots_txt_obey: bool,
}

impl CsvFeedSpider {
    /// Start building a `CsvFeedSpider` with the given spider name.
    pub fn builder(name: &str) -> CsvFeedSpiderBuilder {
        CsvFeedSpiderBuilder::new(name)
    }
}

/// Parse an RFC 4180 CSV document into rows of fields. Handles quoted
/// fields (with `""` escapes and embedded delimiters/newlines), LF and
/// CRLF row endings, and a trailing newline without emitting a phantom
/// empty row.
fn parse_csv(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    // Tracks whether the current row has any content at all, so a trailing
    // newline doesn't produce a phantom ["" ] row while a genuinely empty
    // line mid-file still does.
    let mut row_started = false;

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                c => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                in_quotes = true;
                row_started = true;
            }
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
                row_started = true;
            }
            '\r' => {
                // Consumed as part of CRLF; a bare \r inside an unquoted
                // field is kept verbatim.
                if chars.peek() == Some(&'\n') {
                    continue;
                }
                field.push('\r');
            }
            '\n' => {
                if row_started || !field.is_empty() {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                row_started = false;
            }
            c => {
                field.push(c);
                row_started = true;
            }
        }
    }
    if row_started || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[async_trait]
impl Spider for CsvFeedSpider {
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
        let rows = parse_csv(response.text(), self.delimiter);
        let mut rows_iter = rows.into_iter();

        let headers: Vec<String> = match &self.headers {
            Some(h) => h.clone(),
            None => match rows_iter.next() {
                Some(first) => first,
                None => return (vec![], vec![]),
            },
        };

        let mut items = Vec::new();
        for row in rows_iter {
            let mut obj = serde_json::Map::new();
            for (i, header) in headers.iter().enumerate() {
                let cell = row.get(i).cloned().unwrap_or_default();
                obj.insert(header.clone(), serde_json::Value::String(cell));
            }
            match &self.parse_row {
                Some(f) => items.extend(f(&response, &obj)),
                None => items.push(serde_json::Value::Object(obj)),
            }
        }
        (items, vec![])
    }
}

/// Builder for [`CsvFeedSpider`].
#[must_use = "a builder does nothing until `.build()` is called"]
pub struct CsvFeedSpiderBuilder {
    spider: CsvFeedSpider,
}

impl CsvFeedSpiderBuilder {
    fn new(name: &str) -> Self {
        Self {
            spider: CsvFeedSpider {
                name: name.to_string(),
                feed_urls: Vec::new(),
                delimiter: ',',
                headers: None,
                parse_row: None,
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

    /// Field delimiter (default `,`; use `;` or `\t` for such feeds).
    pub fn delimiter(mut self, delimiter: char) -> Self {
        self.spider.delimiter = delimiter;
        self
    }

    /// Override the column names. When set, the first CSV row is treated
    /// as data instead of a header row.
    pub fn headers(mut self, headers: impl IntoIterator<Item = String>) -> Self {
        self.spider.headers = Some(headers.into_iter().collect());
        self
    }

    /// Callback turning one row object into items; without it, each row
    /// object itself becomes an item.
    pub fn parse_row(mut self, f: ParseRowFn) -> Self {
        self.spider.parse_row = Some(f);
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
    pub fn build(self) -> CsvFeedSpider {
        self.spider
    }
}

#[cfg(test)]
mod tests {
    use super::parse_csv;

    #[test]
    fn parses_quoted_fields_delimiters_and_crlf() {
        let text = "a,b,c\r\n\"x,1\",\"say \"\"hi\"\"\",plain\r\nlast,\"multi\nline\",end\r\n";
        let rows = parse_csv(text, ',');
        assert_eq!(
            rows,
            vec![
                vec!["a", "b", "c"],
                vec!["x,1", "say \"hi\"", "plain"],
                vec!["last", "multi\nline", "end"],
            ]
        );
    }

    #[test]
    fn trailing_newline_produces_no_phantom_row() {
        assert_eq!(
            parse_csv("a,b\n1,2\n", ','),
            vec![vec!["a", "b"], vec!["1", "2"]]
        );
        // No trailing newline works too.
        assert_eq!(
            parse_csv("a,b\n1,2", ','),
            vec![vec!["a", "b"], vec!["1", "2"]]
        );
    }

    #[test]
    fn custom_delimiter_and_empty_fields() {
        assert_eq!(
            parse_csv("a;;c\n;;\n", ';'),
            vec![vec!["a", "", "c"], vec!["", "", ""]]
        );
    }
}
