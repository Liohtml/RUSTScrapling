# RUSTScrapling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Python Scrapling web scraping framework as a native Rust library with the same API surface: HTML parsing with CSS/XPath selectors, adaptive element relocation, multi-strategy HTTP fetching, and a spider-based crawling framework.

**Architecture:** Three-layer design mirroring Python Scrapling: (1) Parser layer with `Selector`/`Selectors` types wrapping `scraper`/`lxml`-equivalent DOM trees, (2) Fetcher layer using `reqwest` for HTTP with TLS fingerprinting and retry logic, (3) Spider layer using `tokio` for async concurrent crawling with rate limiting, deduplication, and checkpointing. Each layer is a separate Rust module that can be used independently.

**Tech Stack:** `scraper` + `selectors` (CSS), `sxd-xpath` or `scraper` built-in (XPath-like), `reqwest` (HTTP), `tokio` (async runtime), `rusqlite` (storage), `serde`/`serde_json` (serialization), `regex` (pattern matching), `sha2` (fingerprinting), `clap` (CLI), `url` (URL handling)

---

## File Structure

```
RUSTScrapling/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Public API re-exports
│   ├── main.rs                     # CLI entry point
│   │
│   ├── core/
│   │   ├── mod.rs                  # Core module exports
│   │   ├── text_handler.rs         # TextHandler type (wraps String with regex/json)
│   │   ├── text_handlers.rs        # TextHandlers (Vec<TextHandler> with batch ops)
│   │   ├── attributes_handler.rs   # AttributesHandler (read-only attribute map)
│   │   └── storage.rs              # SQLite storage system for adaptive mode
│   │
│   ├── parser/
│   │   ├── mod.rs                  # Parser module exports
│   │   ├── selector.rs             # Selector struct (single element wrapper)
│   │   ├── selectors.rs            # Selectors struct (Vec<Selector> with batch ops)
│   │   ├── selector_generation.rs  # Auto-generate CSS/XPath from element position
│   │   └── translator.rs           # CSS-to-XPath with ::text and ::attr() extensions
│   │
│   ├── fetchers/
│   │   ├── mod.rs                  # Fetcher module exports
│   │   ├── config.rs               # Request configuration, headers, stealth logic
│   │   ├── client.rs               # Sync/Async HTTP client (reqwest-based)
│   │   ├── response.rs             # Response wrapper → Selector integration
│   │   ├── proxy.rs                # Proxy rotation
│   │   └── constants.rs            # Browser args, user agents, blocked resource types
│   │
│   └── spiders/
│       ├── mod.rs                  # Spider module exports
│       ├── spider.rs               # Spider trait (user implements this)
│       ├── engine.rs               # CrawlerEngine async orchestrator
│       ├── request.rs              # Request struct with fingerprinting
│       ├── response.rs             # Spider response (wraps fetcher response)
│       ├── result.rs               # CrawlResult, CrawlStats, ItemList
│       ├── scheduler.rs            # Priority queue with dedup
│       ├── session.rs              # Session manager
│       ├── checkpoint.rs           # Pause/resume persistence
│       ├── cache.rs                # Dev-mode response caching
│       └── robots.rs               # robots.txt parser and compliance
│
├── tests/
│   ├── core/
│   │   ├── test_text_handler.rs
│   │   ├── test_attributes_handler.rs
│   │   └── test_storage.rs
│   ├── parser/
│   │   ├── test_selector.rs
│   │   ├── test_selectors.rs
│   │   └── test_selector_generation.rs
│   ├── fetchers/
│   │   ├── test_config.rs
│   │   └── test_client.rs
│   └── spiders/
│       ├── test_request.rs
│       ├── test_scheduler.rs
│       ├── test_result.rs
│       └── test_engine.rs
│
└── docs/
    └── superpowers/
        └── plans/
            └── 2026-05-04-rust-scrapling.md  # This file
```

---

## Task 1: Project Setup & Dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/main.rs`

- [ ] **Step 1: Configure Cargo.toml with all dependencies**

```toml
[package]
name = "rust_scrapling"
version = "0.1.0"
edition = "2021"
description = "A Rust port of Scrapling - modern web scraping framework"

[dependencies]
# HTML parsing
scraper = "0.22"
ego-tree = "0.10"
markup5ever = "0.14"
cssparser = "0.34"

# HTTP
reqwest = { version = "0.12", features = ["json", "cookies", "gzip", "brotli", "deflate", "rustls-tls"] }
http = "1"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Storage
rusqlite = { version = "0.32", features = ["bundled"] }

# Utilities
regex = "1"
sha2 = "0.10"
url = "2"
thiserror = "2"
once_cell = "1"
indexmap = "2"

# CLI
clap = { version = "4", features = ["derive"] }

# Logging
log = "0.4"
env_logger = "0.11"

# robots.txt
texting_robots = "0.2"

[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
tempfile = "3"
```

- [ ] **Step 2: Set up lib.rs with module declarations**

```rust
pub mod core;
pub mod parser;
pub mod fetchers;
pub mod spiders;
```

- [ ] **Step 3: Set up main.rs placeholder**

```rust
use clap::Parser;

fn main() {
    println!("RUSTScrapling CLI - coming soon");
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors (warnings OK)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: initialize RUSTScrapling project with dependencies"
```

---

## Task 2: Core - TextHandler Type

**Files:**
- Create: `src/core/mod.rs`
- Create: `src/core/text_handler.rs`
- Create: `tests/core/test_text_handler.rs`

- [ ] **Step 1: Write failing tests for TextHandler**

Create `tests/core/test_text_handler.rs`:

```rust
use rust_scrapling::core::TextHandler;

#[test]
fn test_text_handler_from_string() {
    let t = TextHandler::new("Hello World");
    assert_eq!(t.as_str(), "Hello World");
}

#[test]
fn test_text_handler_clean() {
    let t = TextHandler::new("Hello\t\r\n  World  ");
    let cleaned = t.clean(false);
    assert_eq!(cleaned.as_str(), "Hello World");
}

#[test]
fn test_text_handler_clean_with_entities() {
    let t = TextHandler::new("Hello &amp; World");
    let cleaned = t.clean(true);
    assert_eq!(cleaned.as_str(), "Hello & World");
}

#[test]
fn test_text_handler_json() {
    let t = TextHandler::new(r#"{"key": "value"}"#);
    let val = t.json().unwrap();
    assert_eq!(val["key"], "value");
}

#[test]
fn test_text_handler_re() {
    let t = TextHandler::new("Price: $19.99 and $29.99");
    let matches = t.re(r"\$(\d+\.\d+)", true, false, true);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].as_str(), "19.99");
    assert_eq!(matches[1].as_str(), "29.99");
}

#[test]
fn test_text_handler_re_first() {
    let t = TextHandler::new("Price: $19.99");
    let first = t.re_first(r"\$(\d+\.\d+)", true, false, true);
    assert_eq!(first.unwrap().as_str(), "19.99");
}

#[test]
fn test_text_handler_re_first_default() {
    let t = TextHandler::new("No match here");
    let first = t.re_first(r"\$(\d+\.\d+)", true, false, true);
    assert!(first.is_none());
}

#[test]
fn test_text_handler_display() {
    let t = TextHandler::new("test");
    assert_eq!(format!("{}", t), "test");
}

#[test]
fn test_text_handler_strip() {
    let t = TextHandler::new("  hello  ");
    assert_eq!(t.strip().as_str(), "hello");
}

#[test]
fn test_text_handler_to_lowercase() {
    let t = TextHandler::new("HELLO");
    assert_eq!(t.to_lowercase().as_str(), "hello");
}

#[test]
fn test_text_handler_to_uppercase() {
    let t = TextHandler::new("hello");
    assert_eq!(t.to_uppercase().as_str(), "HELLO");
}

#[test]
fn test_text_handler_contains() {
    let t = TextHandler::new("Hello World");
    assert!(t.contains_str("World"));
    assert!(!t.contains_str("world"));
}

#[test]
fn test_text_handler_replace() {
    let t = TextHandler::new("Hello World");
    assert_eq!(t.replace_str("World", "Rust").as_str(), "Hello Rust");
}

#[test]
fn test_text_handler_split() {
    let t = TextHandler::new("a,b,c");
    let parts = t.split_str(",");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].as_str(), "a");
}

#[test]
fn test_text_handler_is_empty() {
    let t = TextHandler::new("");
    assert!(t.is_empty());
    let t2 = TextHandler::new("x");
    assert!(!t2.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_text_handler 2>&1 | head -5`
Expected: Compilation error - module not found

- [ ] **Step 3: Create core/mod.rs**

```rust
pub mod text_handler;
pub mod text_handlers;
pub mod attributes_handler;
pub mod storage;

pub use text_handler::TextHandler;
pub use text_handlers::TextHandlers;
pub use attributes_handler::AttributesHandler;
```

- [ ] **Step 4: Implement TextHandler**

Create `src/core/text_handler.rs`:

```rust
use regex::Regex;
use std::fmt;

/// A string wrapper with scraping-specific methods (regex, JSON, cleaning).
/// Mirrors Python Scrapling's TextHandler(str).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TextHandler {
    value: String,
}

impl TextHandler {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn into_string(self) -> String {
        self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Strip whitespace from both ends.
    pub fn strip(&self) -> TextHandler {
        TextHandler::new(self.value.trim())
    }

    pub fn to_lowercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_lowercase())
    }

    pub fn to_uppercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_uppercase())
    }

    pub fn contains_str(&self, pattern: &str) -> bool {
        self.value.contains(pattern)
    }

    pub fn replace_str(&self, from: &str, to: &str) -> TextHandler {
        TextHandler::new(self.value.replace(from, to))
    }

    pub fn split_str(&self, delimiter: &str) -> Vec<TextHandler> {
        self.value
            .split(delimiter)
            .map(TextHandler::new)
            .collect()
    }

    pub fn starts_with_str(&self, prefix: &str) -> bool {
        self.value.starts_with(prefix)
    }

    pub fn ends_with_str(&self, suffix: &str) -> bool {
        self.value.ends_with(suffix)
    }

    /// Clean whitespace: remove tabs, CR, LF, collapse spaces.
    /// If `remove_entities` is true, decode HTML entities.
    pub fn clean(&self, remove_entities: bool) -> TextHandler {
        let mut s = self.value.replace('\t', " ");
        s = s.replace('\r', "");
        s = s.replace('\n', " ");
        // Collapse consecutive spaces
        while s.contains("  ") {
            s = s.replace("  ", " ");
        }
        s = s.trim().to_string();

        if remove_entities {
            s = decode_html_entities(&s);
        }

        TextHandler::new(s)
    }

    /// Parse the text content as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.value)
    }

    /// Apply a regex to the text. Returns captured group 1 if present, else group 0.
    /// `replace_entities`: decode HTML entities before matching.
    /// `clean_match`: strip whitespace from results.
    /// `case_sensitive`: if false, adds (?i) flag.
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        let text = if replace_entities {
            decode_html_entities(&self.value)
        } else {
            self.value.clone()
        };

        let full_pattern = if case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };

        let re = match Regex::new(&full_pattern) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let mut results = Vec::new();
        for caps in re.captures_iter(&text) {
            // If there's a capture group, use group 1; otherwise group 0
            let matched = if caps.len() > 1 {
                caps.get(1).map(|m| m.as_str())
            } else {
                caps.get(0).map(|m| m.as_str())
            };

            if let Some(s) = matched {
                let val = if clean_match { s.trim() } else { s };
                results.push(TextHandler::new(val));
            }
        }
        results
    }

    /// Return the first regex match, or None.
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        self.re(pattern, replace_entities, clean_match, case_sensitive)
            .into_iter()
            .next()
    }

    /// Scrapy-compatible: return self as a single-element get.
    pub fn get(&self) -> &TextHandler {
        self
    }

    /// Scrapy-compatible: return self in a vec.
    pub fn getall(&self) -> Vec<TextHandler> {
        vec![self.clone()]
    }
}

impl fmt::Display for TextHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for TextHandler {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl From<String> for TextHandler {
    fn from(s: String) -> Self {
        TextHandler::new(s)
    }
}

impl From<&str> for TextHandler {
    fn from(s: &str) -> Self {
        TextHandler::new(s)
    }
}

/// Decode common HTML entities.
fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}
```

- [ ] **Step 5: Create placeholder files for other core modules**

Create `src/core/text_handlers.rs`:

```rust
use crate::core::TextHandler;

/// A list of TextHandler values with batch operations.
#[derive(Debug, Clone, Default)]
pub struct TextHandlers {
    items: Vec<TextHandler>,
}

impl TextHandlers {
    pub fn new(items: Vec<TextHandler>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items.first().cloned().or(default)
    }

    pub fn getall(&self) -> &[TextHandler] {
        &self.items
    }

    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> TextHandlers {
        let items: Vec<TextHandler> = self
            .items
            .iter()
            .flat_map(|t| t.re(pattern, replace_entities, clean_match, case_sensitive))
            .collect();
        TextHandlers::new(items)
    }

    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        for item in &self.items {
            if let Some(m) = item.re_first(pattern, replace_entities, clean_match, case_sensitive) {
                return Some(m);
            }
        }
        None
    }
}

impl std::ops::Index<usize> for TextHandlers {
    type Output = TextHandler;
    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

impl IntoIterator for TextHandlers {
    type Item = TextHandler;
    type IntoIter = std::vec::IntoIter<TextHandler>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a TextHandlers {
    type Item = &'a TextHandler;
    type IntoIter = std::slice::Iter<'a, TextHandler>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
```

Create `src/core/attributes_handler.rs` (placeholder):

```rust
// Placeholder - implemented in Task 3
```

Create `src/core/storage.rs` (placeholder):

```rust
// Placeholder - implemented in Task 5
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test`
Expected: All TextHandler tests pass

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: implement TextHandler and TextHandlers core types"
```

---

## Task 3: Core - AttributesHandler Type

**Files:**
- Create: `src/core/attributes_handler.rs`
- Create: `tests/core/test_attributes_handler.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/core/test_attributes_handler.rs`:

```rust
use rust_scrapling::core::AttributesHandler;
use rust_scrapling::core::TextHandler;
use std::collections::HashMap;

#[test]
fn test_attributes_from_hashmap() {
    let mut map = HashMap::new();
    map.insert("class".to_string(), "foo bar".to_string());
    map.insert("id".to_string(), "main".to_string());
    let attrs = AttributesHandler::new(map);
    assert_eq!(attrs.get("class").unwrap().as_str(), "foo bar");
    assert_eq!(attrs.get("id").unwrap().as_str(), "main");
}

#[test]
fn test_attributes_missing_key() {
    let attrs = AttributesHandler::new(HashMap::new());
    assert!(attrs.get("nonexistent").is_none());
}

#[test]
fn test_attributes_contains() {
    let mut map = HashMap::new();
    map.insert("href".to_string(), "/page".to_string());
    let attrs = AttributesHandler::new(map);
    assert!(attrs.contains_key("href"));
    assert!(!attrs.contains_key("src"));
}

#[test]
fn test_attributes_len() {
    let mut map = HashMap::new();
    map.insert("a".to_string(), "1".to_string());
    map.insert("b".to_string(), "2".to_string());
    let attrs = AttributesHandler::new(map);
    assert_eq!(attrs.len(), 2);
}

#[test]
fn test_attributes_iter() {
    let mut map = HashMap::new();
    map.insert("key".to_string(), "val".to_string());
    let attrs = AttributesHandler::new(map);
    let keys: Vec<&str> = attrs.keys().collect();
    assert_eq!(keys, vec!["key"]);
}

#[test]
fn test_attributes_search_values_exact() {
    let mut map = HashMap::new();
    map.insert("class".to_string(), "product-title".to_string());
    map.insert("id".to_string(), "name".to_string());
    let attrs = AttributesHandler::new(map);
    let results: Vec<_> = attrs.search_values("product-title", false).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "class");
}

#[test]
fn test_attributes_search_values_partial() {
    let mut map = HashMap::new();
    map.insert("class".to_string(), "product-title".to_string());
    map.insert("data-type".to_string(), "product-card".to_string());
    let attrs = AttributesHandler::new(map);
    let results: Vec<_> = attrs.search_values("product", true).collect();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_attributes_json_string() {
    let mut map = HashMap::new();
    map.insert("id".to_string(), "test".to_string());
    let attrs = AttributesHandler::new(map);
    let json = attrs.json_string();
    assert!(json.contains("\"id\""));
    assert!(json.contains("\"test\""));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_attributes_handler 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Implement AttributesHandler**

Replace `src/core/attributes_handler.rs`:

```rust
use crate::core::TextHandler;
use indexmap::IndexMap;

/// A read-only mapping of HTML element attributes.
/// All values are wrapped in TextHandler for regex/json capabilities.
#[derive(Debug, Clone)]
pub struct AttributesHandler {
    inner: IndexMap<String, TextHandler>,
}

impl AttributesHandler {
    pub fn new(map: impl IntoIterator<Item = (String, String)>) -> Self {
        let inner: IndexMap<String, TextHandler> = map
            .into_iter()
            .map(|(k, v)| (k, TextHandler::new(v)))
            .collect();
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<&TextHandler> {
        self.inner.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(|k| k.as_str())
    }

    pub fn values(&self) -> impl Iterator<Item = &TextHandler> {
        self.inner.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &TextHandler)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Search for attributes whose values match a keyword.
    /// If `partial` is false, requires exact match.
    /// If `partial` is true, checks if value contains the keyword.
    pub fn search_values<'a>(
        &'a self,
        keyword: &'a str,
        partial: bool,
    ) -> impl Iterator<Item = (&'a str, &'a TextHandler)> {
        self.inner.iter().filter_map(move |(k, v)| {
            let matches = if partial {
                v.as_str().contains(keyword)
            } else {
                v.as_str() == keyword
            };
            if matches {
                Some((k.as_str(), v))
            } else {
                None
            }
        })
    }

    /// Serialize attributes to a JSON string.
    pub fn json_string(&self) -> String {
        let map: IndexMap<&str, &str> = self
            .inner
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        serde_json::to_string(&map).unwrap_or_default()
    }
}

impl std::ops::Index<&str> for AttributesHandler {
    type Output = TextHandler;
    fn index(&self, key: &str) -> &Self::Output {
        &self.inner[key]
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All AttributesHandler tests pass

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement AttributesHandler core type"
```

---

## Task 4: Parser - Selector and Selectors

**Files:**
- Create: `src/parser/mod.rs`
- Create: `src/parser/selector.rs`
- Create: `src/parser/selectors.rs`
- Create: `tests/parser/test_selector.rs`

- [ ] **Step 1: Write failing tests for Selector**

Create `tests/parser/test_selector.rs`:

```rust
use rust_scrapling::parser::{Selector, Selectors};

const HTML: &str = r#"
<html>
<head><title>Test Page</title></head>
<body>
  <div id="main" class="container">
    <h1 class="title">Hello World</h1>
    <p class="description">This is a <strong>test</strong> page.</p>
    <ul id="items">
      <li class="item" data-price="19.99">Item 1</li>
      <li class="item" data-price="29.99">Item 2</li>
      <li class="item special" data-price="39.99">Item 3</li>
    </ul>
    <a href="/next" class="nav-link">Next Page</a>
    <script>var x = 1;</script>
  </div>
</body>
</html>
"#;

#[test]
fn test_selector_from_html() {
    let sel = Selector::from_html(HTML);
    assert!(!sel.tag().is_empty());
}

#[test]
fn test_css_selector() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("h1.title");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text().as_str(), "Hello World");
}

#[test]
fn test_css_selector_multiple() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    assert_eq!(items.len(), 3);
}

#[test]
fn test_css_id_selector() {
    let sel = Selector::from_html(HTML);
    let main = sel.css("#main");
    assert_eq!(main.len(), 1);
    assert_eq!(main[0].tag(), "div");
}

#[test]
fn test_css_attribute_selector() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("[data-price]");
    assert_eq!(results.len(), 3);
}

#[test]
fn test_tag_property() {
    let sel = Selector::from_html(HTML);
    let h1 = sel.css("h1");
    assert_eq!(h1[0].tag(), "h1");
}

#[test]
fn test_text_property() {
    let sel = Selector::from_html(HTML);
    let h1 = sel.css("h1.title");
    assert_eq!(h1[0].text().as_str(), "Hello World");
}

#[test]
fn test_attrib_property() {
    let sel = Selector::from_html(HTML);
    let link = sel.css("a.nav-link");
    assert_eq!(link[0].attrib().get("href").unwrap().as_str(), "/next");
}

#[test]
fn test_html_content() {
    let sel = Selector::from_html(HTML);
    let p = sel.css("p.description");
    let html = p[0].html_content();
    assert!(html.as_str().contains("<strong>"));
}

#[test]
fn test_get_all_text() {
    let sel = Selector::from_html(HTML);
    let p = sel.css("p.description");
    let text = p[0].get_all_text("\n", false, &["script", "style"], true);
    assert!(text.as_str().contains("test"));
    assert!(text.as_str().contains("This is a"));
}

#[test]
fn test_get_all_text_ignores_script() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    let text = div[0].get_all_text(" ", false, &["script", "style"], true);
    assert!(!text.as_str().contains("var x"));
}

#[test]
fn test_children() {
    let sel = Selector::from_html(HTML);
    let ul = sel.css("ul#items");
    let children = ul[0].children();
    assert_eq!(children.len(), 3);
}

#[test]
fn test_parent() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.item");
    let parent = li[0].parent();
    assert!(parent.is_some());
    assert_eq!(parent.unwrap().tag(), "ul");
}

#[test]
fn test_has_class() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.special");
    assert!(li[0].has_class("special"));
    assert!(li[0].has_class("item"));
    assert!(!li[0].has_class("foo"));
}

#[test]
fn test_selectors_css_chaining() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    // No nested CSS needed for this test, but verify chaining works
    assert_eq!(items.len(), 3);
    assert_eq!(items.first().unwrap().text().as_str(), "Item 1");
    assert_eq!(items.last().unwrap().text().as_str(), "Item 3");
}

#[test]
fn test_selectors_get() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let first = items.get_first(None);
    assert!(first.is_some());
}

#[test]
fn test_selectors_getall() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let all = items.getall();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_selectors_filter() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let special = items.filter(|s| s.has_class("special"));
    assert_eq!(special.len(), 1);
}

#[test]
fn test_selectors_search() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let found = items.search(|s| s.text().as_str() == "Item 2");
    assert!(found.is_some());
}

#[test]
fn test_selector_re() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("[data-price]");
    let prices = items[0].re(r"(\d+\.\d+)", true, false, true);
    assert_eq!(prices.len(), 1);
    assert_eq!(prices[0].as_str(), "19.99");
}

#[test]
fn test_selector_json() {
    let json_html = r#"<html><body><script type="application/json">{"name":"test"}</script></body></html>"#;
    let sel = Selector::from_html(json_html);
    let scripts = sel.css("script");
    let val = scripts[0].json();
    assert!(val.is_ok());
    assert_eq!(val.unwrap()["name"], "test");
}

#[test]
fn test_find_by_text() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_text("Hello World", true, false, false);
    assert!(found.is_some());
    assert_eq!(found.unwrap().tag(), "h1");
}

#[test]
fn test_find_by_text_partial() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_text("Hello", true, true, false);
    assert!(found.is_some());
}

#[test]
fn test_find_by_regex() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_regex(r"Item \d+", true, false);
    assert!(found.is_some());
}

#[test]
fn test_urljoin() {
    let sel = Selector::from_html_with_url(HTML, "https://example.com/page/1");
    let abs = sel.urljoin("/next");
    assert_eq!(abs, "https://example.com/next");
}

#[test]
fn test_selector_index_access() {
    let sel = Selector::from_html(HTML);
    let link = sel.css("a");
    let href = &link[0]["href"];
    assert_eq!(href.as_str(), "/next");
}

#[test]
fn test_empty_css_result() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("div.nonexistent");
    assert_eq!(results.len(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_selector 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Create parser/mod.rs**

```rust
pub mod selector;
pub mod selectors;
pub mod selector_generation;
pub mod translator;

pub use selector::Selector;
pub use selectors::Selectors;
```

- [ ] **Step 4: Implement Selector**

Create `src/parser/selector.rs`:

```rust
use crate::core::{AttributesHandler, TextHandler, TextHandlers};
use crate::parser::selectors::Selectors;
use scraper::{Html, Node, ElementRef};
use ego_tree::NodeId;
use std::collections::HashMap;

/// A wrapper around an HTML element with CSS selector, text extraction,
/// regex, and navigation capabilities. Mirrors Python Scrapling's Selector.
#[derive(Clone)]
pub struct Selector {
    tree: Html,
    node_id: NodeId,
    url: String,
}

impl Selector {
    /// Parse an HTML string into a root Selector.
    pub fn from_html(html: &str) -> Self {
        let tree = Html::parse_document(html);
        let node_id = tree.tree.root().id();
        Self {
            tree,
            node_id,
            url: String::new(),
        }
    }

    /// Parse an HTML string with an associated URL.
    pub fn from_html_with_url(html: &str, url: &str) -> Self {
        let tree = Html::parse_document(html);
        let node_id = tree.tree.root().id();
        Self {
            tree,
            node_id,
            url: url.to_string(),
        }
    }

    /// Create a Selector pointing to a specific node in the same tree.
    fn from_node(tree: Html, node_id: NodeId, url: String) -> Self {
        Self { tree, node_id, url }
    }

    /// Create a child Selector sharing the same tree reference.
    fn child_selector(&self, node_id: NodeId) -> Self {
        Self {
            tree: self.tree.clone(),
            node_id,
            url: self.url.clone(),
        }
    }

    fn element_ref(&self) -> Option<ElementRef<'_>> {
        ElementRef::wrap(self.tree.tree.get(self.node_id)?)
    }

    /// The tag name of this element.
    pub fn tag(&self) -> &str {
        match self.element_ref() {
            Some(el) => el.value().name(),
            None => {
                // Check if it's the root document node
                let node = self.tree.tree.get(self.node_id);
                match node {
                    Some(n) => match n.value() {
                        Node::Document => "html",
                        Node::Text(_) => "#text",
                        _ => "",
                    },
                    None => "",
                }
            }
        }
    }

    /// Direct text content of this element (not recursive).
    pub fn text(&self) -> TextHandler {
        if let Some(el) = self.element_ref() {
            let text: String = el
                .children()
                .filter_map(|child| match child.value() {
                    Node::Text(t) => Some(t.text.as_ref()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            TextHandler::new(text.trim())
        } else {
            // For document root, get text of the first element child
            TextHandler::new("")
        }
    }

    /// Element attributes as an AttributesHandler.
    pub fn attrib(&self) -> AttributesHandler {
        if let Some(el) = self.element_ref() {
            let map: HashMap<String, String> = el
                .value()
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            AttributesHandler::new(map)
        } else {
            AttributesHandler::new(std::iter::empty::<(String, String)>())
        }
    }

    /// Inner HTML content as a string.
    pub fn html_content(&self) -> TextHandler {
        if let Some(el) = self.element_ref() {
            TextHandler::new(el.inner_html())
        } else {
            TextHandler::new("")
        }
    }

    /// Outer HTML of this element.
    pub fn outer_html(&self) -> TextHandler {
        if let Some(el) = self.element_ref() {
            TextHandler::new(el.html())
        } else {
            TextHandler::new("")
        }
    }

    /// Recursively get all visible text, ignoring specified tags.
    pub fn get_all_text(
        &self,
        separator: &str,
        strip: bool,
        ignore_tags: &[&str],
        valid_values: bool,
    ) -> TextHandler {
        let mut texts = Vec::new();
        self.collect_text(&mut texts, ignore_tags);

        let result: Vec<String> = texts
            .into_iter()
            .map(|t| if strip { t.trim().to_string() } else { t })
            .filter(|t| !valid_values || !t.trim().is_empty())
            .collect();

        TextHandler::new(result.join(separator))
    }

    fn collect_text(&self, texts: &mut Vec<String>, ignore_tags: &[&str]) {
        let node = match self.tree.tree.get(self.node_id) {
            Some(n) => n,
            None => return,
        };

        for child in node.children() {
            match child.value() {
                Node::Text(t) => {
                    texts.push(t.text.to_string());
                }
                Node::Element(el) => {
                    if !ignore_tags.contains(&el.name.local.as_ref()) {
                        let child_sel = self.child_selector(child.id());
                        child_sel.collect_text(texts, ignore_tags);
                    }
                }
                _ => {}
            }
        }
    }

    /// CSS selector query. Returns all matching descendants.
    pub fn css(&self, selector: &str) -> Selectors {
        let css_selector = match scraper::Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => return Selectors::new(vec![]),
        };

        if let Some(el) = self.element_ref() {
            let results: Vec<Selector> = el
                .select(&css_selector)
                .map(|matched| self.child_selector(matched.id()))
                .collect();
            Selectors::new(results)
        } else {
            // For document root node, select from the tree directly
            let results: Vec<Selector> = self
                .tree
                .select(&css_selector)
                .map(|matched| self.child_selector(matched.id()))
                .collect();
            Selectors::new(results)
        }
    }

    /// Direct children of this element (elements only, not text/comments).
    pub fn children(&self) -> Selectors {
        let node = match self.tree.tree.get(self.node_id) {
            Some(n) => n,
            None => return Selectors::new(vec![]),
        };

        let results: Vec<Selector> = node
            .children()
            .filter(|child| matches!(child.value(), Node::Element(_)))
            .map(|child| self.child_selector(child.id()))
            .collect();
        Selectors::new(results)
    }

    /// Parent element, if any.
    pub fn parent(&self) -> Option<Selector> {
        let node = self.tree.tree.get(self.node_id)?;
        let parent = node.parent()?;
        if matches!(parent.value(), Node::Element(_)) {
            Some(self.child_selector(parent.id()))
        } else {
            None
        }
    }

    /// Sibling elements (other children of this element's parent).
    pub fn siblings(&self) -> Selectors {
        if let Some(parent) = self.parent() {
            let results: Vec<Selector> = parent
                .children()
                .into_iter()
                .filter(|s| s.node_id != self.node_id)
                .collect();
            Selectors::new(results)
        } else {
            Selectors::new(vec![])
        }
    }

    /// Next sibling element.
    pub fn next(&self) -> Option<Selector> {
        let node = self.tree.tree.get(self.node_id)?;
        let mut current = node.next_sibling();
        while let Some(sibling) = current {
            if matches!(sibling.value(), Node::Element(_)) {
                return Some(self.child_selector(sibling.id()));
            }
            current = sibling.next_sibling();
        }
        None
    }

    /// Previous sibling element.
    pub fn previous(&self) -> Option<Selector> {
        let node = self.tree.tree.get(self.node_id)?;
        let mut current = node.prev_sibling();
        while let Some(sibling) = current {
            if matches!(sibling.value(), Node::Element(_)) {
                return Some(self.child_selector(sibling.id()));
            }
            current = sibling.prev_sibling();
        }
        None
    }

    /// Check if this element has a specific CSS class.
    pub fn has_class(&self, class_name: &str) -> bool {
        if let Some(el) = self.element_ref() {
            el.value()
                .attr("class")
                .map(|c| c.split_whitespace().any(|cls| cls == class_name))
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Join a relative URL with this Selector's base URL.
    pub fn urljoin(&self, relative_url: &str) -> String {
        if self.url.is_empty() {
            return relative_url.to_string();
        }
        match url::Url::parse(&self.url) {
            Ok(base) => match base.join(relative_url) {
                Ok(abs) => abs.to_string(),
                Err(_) => relative_url.to_string(),
            },
            Err(_) => relative_url.to_string(),
        }
    }

    /// Apply regex to this element's text content.
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        self.text()
            .re(pattern, replace_entities, clean_match, case_sensitive)
    }

    /// First regex match on this element's text.
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        self.text()
            .re_first(pattern, replace_entities, clean_match, case_sensitive)
    }

    /// Parse text content as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        self.text().json()
    }

    /// Find first element matching text.
    pub fn find_by_text(
        &self,
        text: &str,
        first_match: bool,
        partial: bool,
        case_sensitive: bool,
    ) -> Option<Selector> {
        let results = self.find_all_by_text(text, partial, case_sensitive);
        if first_match {
            results.into_iter().next()
        } else {
            results.into_iter().next()
        }
    }

    /// Find all elements matching text.
    pub fn find_all_by_text(
        &self,
        text: &str,
        partial: bool,
        case_sensitive: bool,
    ) -> Selectors {
        let all = self.css("*");
        let results: Vec<Selector> = all
            .into_iter()
            .filter(|el| {
                let el_text = el.text();
                let el_str = el_text.as_str();
                if case_sensitive {
                    if partial {
                        el_str.contains(text)
                    } else {
                        el_str == text
                    }
                } else {
                    let lower_el = el_str.to_lowercase();
                    let lower_text = text.to_lowercase();
                    if partial {
                        lower_el.contains(&lower_text)
                    } else {
                        lower_el == lower_text
                    }
                }
            })
            .collect();
        Selectors::new(results)
    }

    /// Find first element matching a regex against its text.
    pub fn find_by_regex(
        &self,
        pattern: &str,
        first_match: bool,
        case_sensitive: bool,
    ) -> Option<Selector> {
        let full_pattern = if case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };
        let re = regex::Regex::new(&full_pattern).ok()?;

        let all = self.css("*");
        for el in all.into_iter() {
            let el_text = el.text();
            if re.is_match(el_text.as_str()) {
                if first_match {
                    return Some(el);
                }
            }
        }
        None
    }

    /// Access attribute by key via indexing: `selector["href"]`.
    pub fn get_attribute(&self, key: &str) -> Option<TextHandler> {
        self.attrib().get(key).cloned()
    }
}

impl std::ops::Index<&str> for Selector {
    type Output = TextHandler;

    fn index(&self, key: &str) -> &Self::Output {
        // This is a workaround since we can't return a reference to a temporary.
        // In practice, users should use .attrib().get(key) for Option-based access.
        // For indexing, we leak a static reference (safe for scraping use).
        // A better approach is to cache attributes.
        panic!(
            "Use selector.attrib().get(\"{}\") instead of indexing - Index trait requires &Self::Output which can't reference temporaries",
            key
        )
    }
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Selector(<{}>)", self.tag())
    }
}
```

- [ ] **Step 5: Implement Selectors**

Create `src/parser/selectors.rs`:

```rust
use crate::core::TextHandler;
use crate::parser::selector::Selector;

/// A list of Selector elements with batch operations.
/// Mirrors Python Scrapling's Selectors(List[Selector]).
#[derive(Debug, Clone)]
pub struct Selectors {
    items: Vec<Selector>,
}

impl Selectors {
    pub fn new(items: Vec<Selector>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn first(&self) -> Option<&Selector> {
        self.items.first()
    }

    pub fn last(&self) -> Option<&Selector> {
        self.items.last()
    }

    /// CSS selector across all elements, flattened.
    pub fn css(&self, selector: &str) -> Selectors {
        let results: Vec<Selector> = self
            .items
            .iter()
            .flat_map(|s| s.css(selector).into_iter())
            .collect();
        Selectors::new(results)
    }

    /// Regex across all elements, flattened.
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        self.items
            .iter()
            .flat_map(|s| s.re(pattern, replace_entities, clean_match, case_sensitive))
            .collect()
    }

    /// First regex match across all elements.
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        for item in &self.items {
            if let Some(m) = item.re_first(pattern, replace_entities, clean_match, case_sensitive) {
                return Some(m);
            }
        }
        None
    }

    /// First element's serialized text, or default.
    pub fn get_first(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items.first().map(|s| s.text()).or(default)
    }

    /// All elements' text as TextHandlers.
    pub fn getall(&self) -> Vec<TextHandler> {
        self.items.iter().map(|s| s.text()).collect()
    }

    /// Find first element matching predicate.
    pub fn search(&self, func: impl Fn(&Selector) -> bool) -> Option<&Selector> {
        self.items.iter().find(|s| func(s))
    }

    /// Filter elements by predicate.
    pub fn filter(&self, func: impl Fn(&Selector) -> bool) -> Selectors {
        let items: Vec<Selector> = self.items.iter().filter(|s| func(s)).cloned().collect();
        Selectors::new(items)
    }
}

impl std::ops::Index<usize> for Selectors {
    type Output = Selector;
    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

impl IntoIterator for Selectors {
    type Item = Selector;
    type IntoIter = std::vec::IntoIter<Selector>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a Selectors {
    type Item = &'a Selector;
    type IntoIter = std::slice::Iter<'a, Selector>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
```

- [ ] **Step 6: Create parser placeholders**

Create `src/parser/selector_generation.rs`:

```rust
// Implemented in Task 6
```

Create `src/parser/translator.rs`:

```rust
// Implemented in Task 6
```

- [ ] **Step 7: Fix the Index trait issue for Selector**

The `Index<&str>` trait on Selector won't work cleanly because we can't return a reference to a temporary `TextHandler`. Instead, remove the `Index` impl and update the test to use `.attrib().get()`.

Update the test `test_selector_index_access` to:

```rust
#[test]
fn test_selector_attribute_access() {
    let sel = Selector::from_html(HTML);
    let link = sel.css("a");
    let href = link[0].attrib().get("href").unwrap().clone();
    assert_eq!(href.as_str(), "/next");
}
```

Remove the `Index<&str>` impl from `src/parser/selector.rs`.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test`
Expected: All parser tests pass

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: implement Selector and Selectors parser types with CSS, text, and navigation"
```

---

## Task 5: Core - SQLite Storage System

**Files:**
- Modify: `src/core/storage.rs`
- Create: `tests/core/test_storage.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/core/test_storage.rs`:

```rust
use rust_scrapling::core::storage::SqliteStorage;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_storage_save_and_retrieve() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let mut element_data: HashMap<String, serde_json::Value> = HashMap::new();
    element_data.insert("tag".into(), serde_json::json!("div"));
    element_data.insert("class".into(), serde_json::json!("product"));
    element_data.insert("text".into(), serde_json::json!("Hello"));

    storage.save("product-title", &element_data).unwrap();

    let retrieved = storage.retrieve("product-title").unwrap();
    assert!(retrieved.is_some());
    let data = retrieved.unwrap();
    assert_eq!(data["tag"], "div");
    assert_eq!(data["class"], "product");
}

#[test]
fn test_storage_retrieve_nonexistent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let result = storage.retrieve("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_storage_update_existing() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let mut data1: HashMap<String, serde_json::Value> = HashMap::new();
    data1.insert("tag".into(), serde_json::json!("div"));
    storage.save("el1", &data1).unwrap();

    let mut data2: HashMap<String, serde_json::Value> = HashMap::new();
    data2.insert("tag".into(), serde_json::json!("span"));
    storage.save("el1", &data2).unwrap();

    let retrieved = storage.retrieve("el1").unwrap().unwrap();
    assert_eq!(retrieved["tag"], "span");
}

#[test]
fn test_storage_different_urls() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage1 = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();
    let storage2 = SqliteStorage::new(db_path.to_str().unwrap(), "https://other.com").unwrap();

    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    data.insert("tag".into(), serde_json::json!("div"));
    storage1.save("el1", &data).unwrap();

    let result = storage2.retrieve("el1").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_storage_identifier_hashing() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    data.insert("tag".into(), serde_json::json!("div"));

    // Different identifiers should not collide
    storage.save("selector-1", &data).unwrap();
    storage.save("selector-2", &data).unwrap();

    assert!(storage.retrieve("selector-1").unwrap().is_some());
    assert!(storage.retrieve("selector-2").unwrap().is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_storage 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Implement SqliteStorage**

Replace `src/core/storage.rs`:

```rust
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

/// SQLite-backed storage system for adaptive element relocation.
/// Stores element properties keyed by (url, identifier) pairs.
pub struct SqliteStorage {
    conn: Mutex<Connection>,
    url: String,
}

impl SqliteStorage {
    /// Create a new storage system backed by a SQLite database file.
    pub fn new(db_path: &str, url: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;

        // Enable WAL for better concurrency
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create storage table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS storage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                identifier TEXT NOT NULL,
                element_data TEXT NOT NULL,
                UNIQUE(url, identifier)
            )",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            url: url.to_lowercase(),
        })
    }

    /// Save element data under a given identifier.
    /// Uses INSERT OR REPLACE to update if the (url, identifier) already exists.
    pub fn save(
        &self,
        identifier: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<(), StorageError> {
        let hash = Self::get_hash(identifier);
        let json = serde_json::to_string(data)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO storage (url, identifier, element_data) VALUES (?1, ?2, ?3)",
            params![self.url, hash, json],
        )?;
        Ok(())
    }

    /// Retrieve stored element data for a given identifier.
    pub fn retrieve(
        &self,
        identifier: &str,
    ) -> Result<Option<HashMap<String, serde_json::Value>>, StorageError> {
        let hash = Self::get_hash(identifier);
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT element_data FROM storage WHERE url = ?1 AND identifier = ?2")?;
        let result: Option<String> = stmt
            .query_row(params![self.url, hash], |row| row.get(0))
            .ok();

        match result {
            Some(json) => {
                let data: HashMap<String, serde_json::Value> = serde_json::from_str(&json)?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    /// Hash an identifier with SHA-256 + length suffix for collision resistance.
    fn get_hash(identifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(identifier.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        format!("{}_{}", hash, identifier.len())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All storage tests pass

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement SQLite storage system for adaptive element tracking"
```

---

## Task 6: Parser - Selector Generation & CSS Translator

**Files:**
- Modify: `src/parser/selector_generation.rs`
- Modify: `src/parser/translator.rs`
- Create: `tests/parser/test_selector_generation.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/parser/test_selector_generation.rs`:

```rust
use rust_scrapling::parser::Selector;
use rust_scrapling::parser::selector_generation::generate_css_selector;
use rust_scrapling::parser::selector_generation::generate_xpath_selector;

const HTML: &str = r#"
<html>
<body>
  <div id="main">
    <ul>
      <li class="item">First</li>
      <li class="item">Second</li>
      <li class="item">Third</li>
    </ul>
    <div>
      <p>Nested para</p>
    </div>
  </div>
</body>
</html>
"#;

#[test]
fn test_generate_css_selector_with_id() {
    let sel = Selector::from_html(HTML);
    let main_div = sel.css("#main");
    let css = generate_css_selector(&main_div[0], false);
    assert!(css.contains("#main"));
}

#[test]
fn test_generate_css_selector_for_li() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let css = generate_css_selector(&items[1], false);
    // Should anchor at #main and use nth-of-type
    assert!(css.contains("li:nth-of-type(2)") || css.contains("#main"));
}

#[test]
fn test_generate_xpath_selector_with_id() {
    let sel = Selector::from_html(HTML);
    let main_div = sel.css("#main");
    let xpath = generate_xpath_selector(&main_div[0], false);
    assert!(xpath.contains("@id='main'"));
}

#[test]
fn test_generate_full_css_selector() {
    let sel = Selector::from_html(HTML);
    let p = sel.css("p");
    let css = generate_css_selector(&p[0], true);
    // Full path should include html > body > div > div > p
    assert!(css.contains("body"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_selector_generation 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Implement selector_generation.rs**

Replace `src/parser/selector_generation.rs`:

```rust
use crate::parser::selector::Selector;

/// Generate a CSS selector for the given element.
/// If `full_path` is false, stops traversal at the nearest ancestor with an `id`.
/// If `full_path` is true, generates a complete path from root.
pub fn generate_css_selector(selector: &Selector, full_path: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(selector.clone());

    while let Some(el) = current {
        let tag = el.tag().to_string();
        if tag.is_empty() || tag == "html" {
            break;
        }

        let attrib = el.attrib();

        // Check for id attribute
        if let Some(id) = attrib.get("id") {
            if !full_path {
                parts.push(format!("#{}", id.as_str()));
                break;
            } else {
                let position = get_nth_of_type(&el);
                if position > 0 {
                    parts.push(format!("{}:nth-of-type({})", tag, position));
                } else {
                    parts.push(tag);
                }
            }
        } else {
            let position = get_nth_of_type(&el);
            if position > 0 {
                parts.push(format!("{}:nth-of-type({})", tag, position));
            } else {
                parts.push(tag);
            }
        }

        current = el.parent();
    }

    parts.reverse();
    parts.join(" > ")
}

/// Generate an XPath selector for the given element.
/// If `full_path` is false, stops at nearest ancestor with an `id`.
pub fn generate_xpath_selector(selector: &Selector, full_path: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(selector.clone());

    while let Some(el) = current {
        let tag = el.tag().to_string();
        if tag.is_empty() || tag == "html" {
            break;
        }

        let attrib = el.attrib();

        if let Some(id) = attrib.get("id") {
            if !full_path {
                parts.push(format!("{}[@id='{}']", tag, id.as_str()));
                break;
            } else {
                let position = get_nth_of_type(&el);
                if position > 0 {
                    parts.push(format!("{}[{}]", tag, position));
                } else {
                    parts.push(tag);
                }
            }
        } else {
            let position = get_nth_of_type(&el);
            if position > 0 {
                parts.push(format!("{}[{}]", tag, position));
            } else {
                parts.push(tag);
            }
        }

        current = el.parent();
    }

    parts.reverse();
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("//{}", parts.join("/"))
    }
}

/// Count the position of this element among siblings with the same tag.
/// Returns 0 if there's only one element of that type (no disambiguation needed).
fn get_nth_of_type(selector: &Selector) -> usize {
    let tag = selector.tag();
    if let Some(parent) = selector.parent() {
        let children = parent.children();
        let same_tag: Vec<_> = children.into_iter().filter(|c| c.tag() == tag).collect();
        if same_tag.len() <= 1 {
            return 0;
        }
        for (i, child) in same_tag.iter().enumerate() {
            if child.text().as_str() == selector.text().as_str()
                && child.attrib().len() == selector.attrib().len()
            {
                return i + 1;
            }
        }
    }
    0
}
```

- [ ] **Step 4: Implement translator.rs (CSS pseudo-elements)**

Replace `src/parser/translator.rs`:

```rust
/// CSS-to-XPath translator with ::text and ::attr() pseudo-element support.
/// This extends standard CSS selectors with Scrapy-compatible pseudo-elements.

/// Process a CSS selector that may contain ::text or ::attr() pseudo-elements.
/// Returns the selector with pseudo-elements stripped, plus extraction info.
pub struct CssQuery {
    /// The CSS selector without pseudo-elements.
    pub selector: String,
    /// If ::text was used, extract text content.
    pub extract_text: bool,
    /// If ::attr(name) was used, extract this attribute.
    pub extract_attr: Option<String>,
}

/// Parse a CSS selector, extracting any ::text or ::attr() pseudo-elements.
pub fn parse_css_query(selector: &str) -> CssQuery {
    let trimmed = selector.trim();

    // Check for ::text pseudo-element
    if let Some(base) = trimmed.strip_suffix("::text") {
        return CssQuery {
            selector: base.trim().to_string(),
            extract_text: true,
            extract_attr: None,
        };
    }

    // Check for ::attr(name) pseudo-element
    if let Some(rest) = trimmed.strip_suffix(')') {
        if let Some(idx) = rest.rfind("::attr(") {
            let base = &rest[..idx];
            let attr_name = &rest[idx + 7..];
            return CssQuery {
                selector: base.trim().to_string(),
                extract_text: false,
                extract_attr: Some(attr_name.trim().to_string()),
            };
        }
    }

    CssQuery {
        selector: trimmed.to_string(),
        extract_text: false,
        extract_attr: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_selector() {
        let q = parse_css_query("div.class");
        assert_eq!(q.selector, "div.class");
        assert!(!q.extract_text);
        assert!(q.extract_attr.is_none());
    }

    #[test]
    fn test_text_pseudo() {
        let q = parse_css_query("h1::text");
        assert_eq!(q.selector, "h1");
        assert!(q.extract_text);
    }

    #[test]
    fn test_attr_pseudo() {
        let q = parse_css_query("a::attr(href)");
        assert_eq!(q.selector, "a");
        assert_eq!(q.extract_attr.unwrap(), "href");
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement CSS/XPath selector generation and CSS pseudo-element translator"
```

---

## Task 7: Fetchers - Configuration & Constants

**Files:**
- Create: `src/fetchers/mod.rs`
- Create: `src/fetchers/config.rs`
- Create: `src/fetchers/constants.rs`
- Create: `src/fetchers/proxy.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/fetchers/test_config.rs`:

```rust
use rust_scrapling::fetchers::config::FetcherConfig;

#[test]
fn test_default_config() {
    let config = FetcherConfig::default();
    assert_eq!(config.timeout_secs, 30);
    assert_eq!(config.retries, 3);
    assert_eq!(config.retry_delay_secs, 1);
    assert!(config.follow_redirects);
    assert_eq!(config.max_redirects, 30);
    assert!(config.verify_ssl);
}

#[test]
fn test_config_builder() {
    let config = FetcherConfig::builder()
        .timeout(60)
        .retries(5)
        .proxy("http://proxy:8080")
        .build();
    assert_eq!(config.timeout_secs, 60);
    assert_eq!(config.retries, 5);
    assert_eq!(config.proxy.as_deref(), Some("http://proxy:8080"));
}

#[test]
fn test_stealth_headers() {
    let config = FetcherConfig::default();
    let headers = config.build_headers("https://example.com", true);
    assert!(headers.contains_key("user-agent"));
    assert!(headers.contains_key("accept"));
    assert!(headers.contains_key("accept-language"));
}

#[test]
fn test_non_stealth_headers() {
    let config = FetcherConfig::default();
    let headers = config.build_headers("https://example.com", false);
    // Should still have user-agent but fewer stealth headers
    assert!(headers.contains_key("user-agent"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_config 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Create fetchers/mod.rs**

```rust
pub mod config;
pub mod constants;
pub mod client;
pub mod response;
pub mod proxy;
```

- [ ] **Step 4: Implement constants.rs**

```rust
/// Resource types to block for faster page loads.
pub const BLOCKED_RESOURCE_TYPES: &[&str] = &[
    "font",
    "image",
    "media",
    "beacon",
    "object",
    "imageset",
    "texttrack",
    "websocket",
    "csp_report",
    "stylesheet",
];

/// Default user agents for impersonation.
pub const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:133.0) Gecko/20100101 Firefox/133.0",
];

/// HTTP status codes that indicate a blocked request.
pub const BLOCKED_STATUS_CODES: &[u16] = &[401, 403, 407, 429, 444, 500, 502, 503, 504];

/// Common accept header value.
pub const ACCEPT_HEADER: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";

/// Common accept-language header value.
pub const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

/// Common accept-encoding header value.
pub const ACCEPT_ENCODING: &str = "gzip, deflate, br";
```

- [ ] **Step 5: Implement config.rs**

```rust
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
            max_redirects: 30,
            verify_ssl: true,
            proxy: None,
            proxies: HashMap::new(),
            headers: HashMap::new(),
            stealthy_headers: true,
            user_agent: None,
        }
    }
}

impl FetcherConfig {
    pub fn builder() -> FetcherConfigBuilder {
        FetcherConfigBuilder::default()
    }

    /// Build HTTP headers for a request.
    /// When `stealth` is true, adds realistic browser headers.
    pub fn build_headers(&self, url: &str, stealth: bool) -> HashMap<String, String> {
        let mut headers = self.headers.clone();

        // Always set user-agent
        let ua = self
            .user_agent
            .clone()
            .unwrap_or_else(|| Self::random_user_agent().to_string());
        headers
            .entry("user-agent".to_string())
            .or_insert(ua);

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
                    r#""Chromium";v="131", "Not_A Brand";v="24""#.to_string()
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
                .entry("upgrade-insecure-requests".to_string())
                .or_insert_with(|| "1".to_string());

            // Add Google referer for Google-related domains
            if url.contains("google.com") {
                headers
                    .entry("referer".to_string())
                    .or_insert_with(|| "https://www.google.com/".to_string());
            }
        }

        headers
    }

    fn random_user_agent() -> &'static str {
        use std::time::{SystemTime, UNIX_EPOCH};
        let idx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as usize
            % constants::USER_AGENTS.len();
        constants::USER_AGENTS[idx]
    }
}

#[derive(Default)]
pub struct FetcherConfigBuilder {
    config: FetcherConfig,
}

impl FetcherConfigBuilder {
    pub fn timeout(mut self, secs: u64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    pub fn retries(mut self, n: u32) -> Self {
        self.config.retries = n;
        self
    }

    pub fn retry_delay(mut self, secs: u64) -> Self {
        self.config.retry_delay_secs = secs;
        self
    }

    pub fn proxy(mut self, proxy: &str) -> Self {
        self.config.proxy = Some(proxy.to_string());
        self
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.config.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn user_agent(mut self, ua: &str) -> Self {
        self.config.user_agent = Some(ua.to_string());
        self
    }

    pub fn stealth(mut self, enabled: bool) -> Self {
        self.config.stealthy_headers = enabled;
        self
    }

    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.config.follow_redirects = follow;
        self
    }

    pub fn verify_ssl(mut self, verify: bool) -> Self {
        self.config.verify_ssl = verify;
        self
    }

    pub fn build(self) -> FetcherConfig {
        self.config
    }
}
```

- [ ] **Step 6: Implement proxy.rs**

```rust
use std::time::{SystemTime, UNIX_EPOCH};

/// A proxy rotator that cycles through a list of proxy URLs.
#[derive(Debug, Clone)]
pub struct ProxyRotator {
    proxies: Vec<String>,
    index: usize,
}

impl ProxyRotator {
    pub fn new(proxies: Vec<String>) -> Self {
        Self { proxies, index: 0 }
    }

    /// Get the next proxy in the rotation.
    pub fn next(&mut self) -> Option<&str> {
        if self.proxies.is_empty() {
            return None;
        }
        let proxy = &self.proxies[self.index % self.proxies.len()];
        self.index += 1;
        Some(proxy)
    }

    /// Get a random proxy from the list.
    pub fn random(&self) -> Option<&str> {
        if self.proxies.is_empty() {
            return None;
        }
        let idx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as usize
            % self.proxies.len();
        Some(&self.proxies[idx])
    }

    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }
}
```

- [ ] **Step 7: Create placeholder files**

Create `src/fetchers/client.rs`:

```rust
// Implemented in Task 8
```

Create `src/fetchers/response.rs`:

```rust
// Implemented in Task 8
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test`
Expected: All config tests pass

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: implement fetcher configuration, constants, and proxy rotation"
```

---

## Task 8: Fetchers - HTTP Client & Response

**Files:**
- Modify: `src/fetchers/client.rs`
- Modify: `src/fetchers/response.rs`
- Create: `tests/fetchers/test_client.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/fetchers/test_client.rs`:

```rust
use rust_scrapling::fetchers::client::Fetcher;
use rust_scrapling::fetchers::config::FetcherConfig;

#[tokio::test]
async fn test_fetcher_get() {
    // Use httpbin or a mock server
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher.get("https://httpbin.org/get").await;
    assert!(response.is_ok());
    let resp = response.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(!resp.text().is_empty());
}

#[tokio::test]
async fn test_fetcher_response_has_selector() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher.get("https://httpbin.org/html").await;
    assert!(response.is_ok());
    let resp = response.unwrap();
    let sel = resp.selector();
    // httpbin /html has an h1 element
    let h1 = sel.css("h1");
    assert!(!h1.is_empty());
}

#[tokio::test]
async fn test_fetcher_post() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher
        .post("https://httpbin.org/post", Some(r#"{"test": true}"#), None)
        .await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap().status(), 200);
}

#[tokio::test]
async fn test_fetcher_with_custom_headers() {
    let config = FetcherConfig::builder()
        .header("X-Custom", "test-value")
        .build();
    let fetcher = Fetcher::new(config);
    let response = fetcher.get("https://httpbin.org/headers").await;
    assert!(response.is_ok());
    let text = response.unwrap().text();
    assert!(text.contains("X-Custom"));
}

#[tokio::test]
async fn test_fetcher_retries_on_error() {
    let config = FetcherConfig::builder()
        .timeout(1)
        .retries(2)
        .retry_delay(0)
        .build();
    let fetcher = Fetcher::new(config);
    // This should fail but retry
    let response = fetcher.get("https://httpbin.org/delay/10").await;
    assert!(response.is_err());
}

#[test]
fn test_response_struct() {
    use rust_scrapling::fetchers::response::Response;
    let resp = Response::new(200, "text/html".to_string(), "<html><body><h1>Test</h1></body></html>".to_string(), "https://example.com".to_string(), std::collections::HashMap::new());
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text(), "<html><body><h1>Test</h1></body></html>");
    let sel = resp.selector();
    let h1 = sel.css("h1");
    assert_eq!(h1[0].text().as_str(), "Test");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_client 2>&1 | head -5`
Expected: Compilation error

- [ ] **Step 3: Implement Response**

Replace `src/fetchers/response.rs`:

```rust
use crate::parser::Selector;
use std::collections::HashMap;

/// HTTP response wrapper that integrates with the parser.
#[derive(Debug, Clone)]
pub struct Response {
    status_code: u16,
    content_type: String,
    body: String,
    url: String,
    headers: HashMap<String, String>,
}

impl Response {
    pub fn new(
        status_code: u16,
        content_type: String,
        body: String,
        url: String,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            status_code,
            content_type,
            body,
            url,
            headers,
        }
    }

    pub fn status(&self) -> u16 {
        self.status_code
    }

    pub fn text(&self) -> &str {
        &self.body
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn content_length(&self) -> usize {
        self.body.len()
    }

    /// Parse the response body as a Selector for HTML extraction.
    pub fn selector(&self) -> Selector {
        Selector::from_html_with_url(&self.body, &self.url)
    }

    /// Parse the response body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.body)
    }

    /// Check if the response indicates success (2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// Check if the response indicates a blocked request.
    pub fn is_blocked(&self) -> bool {
        crate::fetchers::constants::BLOCKED_STATUS_CODES.contains(&self.status_code)
    }
}
```

- [ ] **Step 4: Implement HTTP Client**

Replace `src/fetchers/client.rs`:

```rust
use crate::fetchers::config::FetcherConfig;
use crate::fetchers::response::Response;
use std::collections::HashMap;
use std::time::Duration;

/// Async HTTP client with retry logic and stealth headers.
/// Mirrors Python Scrapling's Fetcher.
pub struct Fetcher {
    config: FetcherConfig,
    client: reqwest::Client,
}

impl Fetcher {
    pub fn new(config: FetcherConfig) -> Self {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(config.max_redirects as usize)
            } else {
                reqwest::redirect::Policy::none()
            });

        if !config.verify_ssl {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some(ref proxy_url) = config.proxy {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }

        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());

        Self { config, client }
    }

    /// Perform a GET request with retry logic.
    pub async fn get(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::GET, url, None, None).await
    }

    /// Perform a POST request with optional body.
    pub async fn post(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::POST, url, body, json).await
    }

    /// Perform a PUT request with optional body.
    pub async fn put(
        &self,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::PUT, url, body, json).await
    }

    /// Perform a DELETE request.
    pub async fn delete(&self, url: &str) -> Result<Response, FetcherError> {
        self.request(reqwest::Method::DELETE, url, None, None).await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Response, FetcherError> {
        let headers = self
            .config
            .build_headers(url, self.config.stealthy_headers);

        let mut last_error = None;

        for attempt in 0..=self.config.retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(self.config.retry_delay_secs)).await;
            }

            let mut req = self.client.request(method.clone(), url);

            // Set headers
            for (key, value) in &headers {
                req = req.header(key.as_str(), value.as_str());
            }

            // Set body
            if let Some(b) = body {
                req = req.body(b.to_string());
            }
            if let Some(j) = json {
                req = req.json(j);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let resp_headers: HashMap<String, String> = resp
                        .headers()
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.as_str().to_string(),
                                v.to_str().unwrap_or("").to_string(),
                            )
                        })
                        .collect();
                    let content_type = resp_headers
                        .get("content-type")
                        .cloned()
                        .unwrap_or_default();
                    let final_url = resp.url().to_string();
                    let text = resp.text().await.unwrap_or_default();

                    return Ok(Response::new(
                        status,
                        content_type,
                        text,
                        final_url,
                        resp_headers,
                    ));
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(FetcherError::RequestFailed(
            last_error.map(|e| e.to_string()).unwrap_or_default(),
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetcherError {
    #[error("Request failed after retries: {0}")]
    RequestFailed(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass (network tests may need `--ignored` flag if no internet)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement async HTTP client with retry logic and response-to-selector integration"
```

---

## Task 9: Spiders - Request, Result, and Scheduler

**Files:**
- Create: `src/spiders/mod.rs`
- Create: `src/spiders/request.rs`
- Create: `src/spiders/result.rs`
- Create: `src/spiders/scheduler.rs`
- Create: `tests/spiders/test_request.rs`
- Create: `tests/spiders/test_scheduler.rs`
- Create: `tests/spiders/test_result.rs`

- [ ] **Step 1: Write failing tests for Request**

Create `tests/spiders/test_request.rs`:

```rust
use rust_scrapling::spiders::request::SpiderRequest;

#[test]
fn test_request_basic() {
    let req = SpiderRequest::new("https://example.com");
    assert_eq!(req.url(), "https://example.com");
    assert_eq!(req.priority(), 0);
    assert!(!req.dont_filter());
}

#[test]
fn test_request_with_priority() {
    let req = SpiderRequest::builder("https://example.com")
        .priority(10)
        .build();
    assert_eq!(req.priority(), 10);
}

#[test]
fn test_request_fingerprint() {
    let req1 = SpiderRequest::new("https://example.com");
    let req2 = SpiderRequest::new("https://example.com");
    assert_eq!(req1.fingerprint(), req2.fingerprint());
}

#[test]
fn test_request_different_urls_different_fingerprints() {
    let req1 = SpiderRequest::new("https://example.com/a");
    let req2 = SpiderRequest::new("https://example.com/b");
    assert_ne!(req1.fingerprint(), req2.fingerprint());
}

#[test]
fn test_request_copy() {
    let req = SpiderRequest::builder("https://example.com")
        .priority(5)
        .build();
    let copy = req.copy();
    assert_eq!(copy.url(), req.url());
    assert_eq!(copy.priority(), req.priority());
}

#[test]
fn test_request_domain() {
    let req = SpiderRequest::new("https://www.example.com/page");
    assert_eq!(req.domain(), "www.example.com");
}

#[test]
fn test_request_ordering() {
    let req1 = SpiderRequest::builder("https://a.com").priority(1).build();
    let req2 = SpiderRequest::builder("https://b.com").priority(10).build();
    // Higher priority should be "greater"
    assert!(req2 > req1);
}

#[test]
fn test_request_meta() {
    let mut req = SpiderRequest::new("https://example.com");
    req.set_meta("page", serde_json::json!(1));
    assert_eq!(req.meta("page"), Some(&serde_json::json!(1)));
}
```

- [ ] **Step 2: Write failing tests for Scheduler**

Create `tests/spiders/test_scheduler.rs`:

```rust
use rust_scrapling::spiders::request::SpiderRequest;
use rust_scrapling::spiders::scheduler::Scheduler;

#[test]
fn test_scheduler_enqueue_dequeue() {
    let mut scheduler = Scheduler::new(false, false, false);
    let req = SpiderRequest::new("https://example.com");
    assert!(scheduler.enqueue(req));
    let dequeued = scheduler.dequeue();
    assert!(dequeued.is_some());
    assert_eq!(dequeued.unwrap().url(), "https://example.com");
}

#[test]
fn test_scheduler_dedup() {
    let mut scheduler = Scheduler::new(false, false, false);
    let req1 = SpiderRequest::new("https://example.com");
    let req2 = SpiderRequest::new("https://example.com");
    assert!(scheduler.enqueue(req1));
    assert!(!scheduler.enqueue(req2)); // duplicate, rejected
}

#[test]
fn test_scheduler_dont_filter() {
    let mut scheduler = Scheduler::new(false, false, false);
    let req1 = SpiderRequest::new("https://example.com");
    let mut req2 = SpiderRequest::new("https://example.com");
    req2.set_dont_filter(true);
    assert!(scheduler.enqueue(req1));
    assert!(scheduler.enqueue(req2)); // dont_filter bypasses dedup
}

#[test]
fn test_scheduler_priority_ordering() {
    let mut scheduler = Scheduler::new(false, false, false);
    let low = SpiderRequest::builder("https://low.com").priority(1).build();
    let high = SpiderRequest::builder("https://high.com").priority(10).build();
    scheduler.enqueue(low);
    scheduler.enqueue(high);
    // Higher priority should come out first
    let first = scheduler.dequeue().unwrap();
    assert_eq!(first.url(), "https://high.com");
}

#[test]
fn test_scheduler_is_empty() {
    let scheduler = Scheduler::new(false, false, false);
    assert!(scheduler.is_empty());
}

#[test]
fn test_scheduler_len() {
    let mut scheduler = Scheduler::new(false, false, false);
    scheduler.enqueue(SpiderRequest::new("https://a.com"));
    scheduler.enqueue(SpiderRequest::new("https://b.com"));
    assert_eq!(scheduler.len(), 2);
}
```

- [ ] **Step 3: Write failing tests for Result**

Create `tests/spiders/test_result.rs`:

```rust
use rust_scrapling::spiders::result::{CrawlStats, ItemList};
use tempfile::tempdir;

#[test]
fn test_item_list_push() {
    let mut items = ItemList::new();
    items.push(serde_json::json!({"title": "Test"}));
    assert_eq!(items.len(), 1);
}

#[test]
fn test_item_list_to_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("items.json");
    let mut items = ItemList::new();
    items.push(serde_json::json!({"title": "Test"}));
    items.to_json(path.to_str().unwrap(), false).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Test"));
}

#[test]
fn test_item_list_to_jsonl() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("items.jsonl");
    let mut items = ItemList::new();
    items.push(serde_json::json!({"a": 1}));
    items.push(serde_json::json!({"b": 2}));
    items.to_jsonl(path.to_str().unwrap()).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_crawl_stats_default() {
    let stats = CrawlStats::default();
    assert_eq!(stats.requests_count, 0);
    assert_eq!(stats.failed_requests_count, 0);
    assert_eq!(stats.items_scraped, 0);
}

#[test]
fn test_crawl_stats_increment() {
    let mut stats = CrawlStats::default();
    stats.increment_requests_count();
    stats.increment_requests_count();
    stats.increment_status(200);
    stats.increment_status(200);
    stats.increment_status(404);
    assert_eq!(stats.requests_count, 2);
    assert_eq!(stats.response_status_count[&200], 2);
    assert_eq!(stats.response_status_count[&404], 1);
}

#[test]
fn test_crawl_stats_elapsed() {
    let mut stats = CrawlStats::default();
    stats.start_time = Some(std::time::Instant::now());
    std::thread::sleep(std::time::Duration::from_millis(10));
    stats.end_time = Some(std::time::Instant::now());
    assert!(stats.elapsed_seconds() > 0.0);
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test 2>&1 | head -5`
Expected: Compilation errors

- [ ] **Step 5: Create spiders/mod.rs**

```rust
pub mod request;
pub mod result;
pub mod scheduler;
pub mod spider;
pub mod engine;
pub mod response;
pub mod session;
pub mod checkpoint;
pub mod cache;
pub mod robots;
```

- [ ] **Step 6: Implement SpiderRequest**

Create `src/spiders/request.rs`:

```rust
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

/// A request in the spider crawl, with priority, fingerprinting, and metadata.
#[derive(Debug, Clone)]
pub struct SpiderRequest {
    url: String,
    method: String,
    session_id: String,
    callback_name: Option<String>,
    priority: i32,
    dont_filter: bool,
    meta: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
    body: Option<String>,
    retry_count: u32,
    fingerprint: String,
}

impl SpiderRequest {
    pub fn new(url: &str) -> Self {
        let mut req = Self {
            url: url.to_string(),
            method: "GET".to_string(),
            session_id: String::new(),
            callback_name: None,
            priority: 0,
            dont_filter: false,
            meta: HashMap::new(),
            headers: HashMap::new(),
            body: None,
            retry_count: 0,
            fingerprint: String::new(),
        };
        req.update_fingerprint(false, false, false);
        req
    }

    pub fn builder(url: &str) -> SpiderRequestBuilder {
        SpiderRequestBuilder::new(url)
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn dont_filter(&self) -> bool {
        self.dont_filter
    }

    pub fn set_dont_filter(&mut self, val: bool) {
        self.dont_filter = val;
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = id;
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn callback_name(&self) -> Option<&str> {
        self.callback_name.as_deref()
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn meta(&self, key: &str) -> Option<&serde_json::Value> {
        self.meta.get(key)
    }

    pub fn set_meta(&mut self, key: &str, value: serde_json::Value) {
        self.meta.insert(key.to_string(), value);
    }

    pub fn domain(&self) -> String {
        Url::parse(&self.url)
            .map(|u| u.host_str().unwrap_or("").to_string())
            .unwrap_or_default()
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn update_fingerprint(
        &mut self,
        include_kwargs: bool,
        include_headers: bool,
        keep_fragments: bool,
    ) {
        let mut hasher = Sha256::new();
        hasher.update(self.session_id.as_bytes());
        hasher.update(self.method.as_bytes());

        // Canonical URL (strip fragment unless keep_fragments)
        let canonical = if keep_fragments {
            self.url.clone()
        } else {
            match Url::parse(&self.url) {
                Ok(mut u) => {
                    u.set_fragment(None);
                    u.to_string()
                }
                Err(_) => self.url.clone(),
            }
        };
        hasher.update(canonical.as_bytes());

        if let Some(ref body) = self.body {
            hasher.update(body.as_bytes());
        }

        if include_headers {
            let mut keys: Vec<&String> = self.headers.keys().collect();
            keys.sort();
            for k in keys {
                hasher.update(k.as_bytes());
                hasher.update(self.headers[k].as_bytes());
            }
        }

        self.fingerprint = format!("{:x}", hasher.finalize());
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }
}

impl PartialEq for SpiderRequest {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

impl Eq for SpiderRequest {}

impl PartialOrd for SpiderRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SpiderRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

pub struct SpiderRequestBuilder {
    request: SpiderRequest,
}

impl SpiderRequestBuilder {
    fn new(url: &str) -> Self {
        Self {
            request: SpiderRequest::new(url),
        }
    }

    pub fn method(mut self, method: &str) -> Self {
        self.request.method = method.to_uppercase();
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.request.priority = priority;
        self
    }

    pub fn session_id(mut self, id: &str) -> Self {
        self.request.session_id = id.to_string();
        self
    }

    pub fn callback(mut self, name: &str) -> Self {
        self.request.callback_name = Some(name.to_string());
        self
    }

    pub fn dont_filter(mut self, val: bool) -> Self {
        self.request.dont_filter = val;
        self
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.request
            .headers
            .insert(key.to_string(), value.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.request.body = Some(body.to_string());
        self
    }

    pub fn meta(mut self, key: &str, value: serde_json::Value) -> Self {
        self.request.meta.insert(key.to_string(), value);
        self
    }

    pub fn build(mut self) -> SpiderRequest {
        self.request.update_fingerprint(false, false, false);
        self.request
    }
}
```

- [ ] **Step 7: Implement Scheduler**

Create `src/spiders/scheduler.rs`:

```rust
use crate::spiders::request::SpiderRequest;
use std::collections::{BinaryHeap, HashSet};

/// Priority queue scheduler with fingerprint-based deduplication.
pub struct Scheduler {
    queue: BinaryHeap<SpiderRequest>,
    seen: HashSet<String>,
    include_kwargs: bool,
    include_headers: bool,
    keep_fragments: bool,
}

impl Scheduler {
    pub fn new(include_kwargs: bool, include_headers: bool, keep_fragments: bool) -> Self {
        Self {
            queue: BinaryHeap::new(),
            seen: HashSet::new(),
            include_kwargs,
            include_headers,
            keep_fragments,
        }
    }

    /// Enqueue a request. Returns true if accepted, false if duplicate.
    pub fn enqueue(&mut self, mut request: SpiderRequest) -> bool {
        request.update_fingerprint(self.include_kwargs, self.include_headers, self.keep_fragments);

        if request.dont_filter() || !self.seen.contains(request.fingerprint()) {
            if !request.dont_filter() {
                self.seen.insert(request.fingerprint().to_string());
            }
            self.queue.push(request);
            true
        } else {
            false
        }
    }

    /// Dequeue the highest-priority request.
    pub fn dequeue(&mut self) -> Option<SpiderRequest> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }

    /// Clear all queued requests and seen fingerprints.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.seen.clear();
    }

    /// Get all pending requests (for checkpoint serialization).
    pub fn pending_requests(&self) -> Vec<&SpiderRequest> {
        self.queue.iter().collect()
    }
}
```

- [ ] **Step 8: Implement Result types**

Create `src/spiders/result.rs`:

```rust
use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;

/// A list of scraped items with export capabilities.
#[derive(Debug, Clone, Default)]
pub struct ItemList {
    items: Vec<serde_json::Value>,
}

impl ItemList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: serde_json::Value) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &serde_json::Value> {
        self.items.iter()
    }

    /// Export items as a JSON array file.
    pub fn to_json(&self, path: &str, indent: bool) -> Result<(), std::io::Error> {
        let json = if indent {
            serde_json::to_string_pretty(&self.items)
        } else {
            serde_json::to_string(&self.items)
        }
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(path, json)
    }

    /// Export items as JSON Lines (one JSON object per line).
    pub fn to_jsonl(&self, path: &str) -> Result<(), std::io::Error> {
        let mut file = std::fs::File::create(path)?;
        for item in &self.items {
            let line = serde_json::to_string(item)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}

impl IntoIterator for ItemList {
    type Item = serde_json::Value;
    type IntoIter = std::vec::IntoIter<serde_json::Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Statistics for a crawl run.
#[derive(Debug, Clone, Default)]
pub struct CrawlStats {
    pub requests_count: u64,
    pub concurrent_requests: u32,
    pub concurrent_requests_per_domain: u32,
    pub failed_requests_count: u64,
    pub offsite_requests_count: u64,
    pub robots_disallowed_count: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub response_bytes: u64,
    pub items_scraped: u64,
    pub items_dropped: u64,
    pub blocked_requests_count: u64,
    pub download_delay: f64,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub response_status_count: HashMap<u16, u64>,
    pub domains_response_bytes: HashMap<String, u64>,
    pub sessions_requests_count: HashMap<String, u64>,
    pub custom_stats: HashMap<String, serde_json::Value>,
}

impl CrawlStats {
    pub fn increment_requests_count(&mut self) {
        self.requests_count += 1;
    }

    pub fn increment_status(&mut self, status: u16) {
        *self.response_status_count.entry(status).or_insert(0) += 1;
    }

    pub fn increment_response_bytes(&mut self, bytes: u64, domain: &str) {
        self.response_bytes += bytes;
        *self
            .domains_response_bytes
            .entry(domain.to_string())
            .or_insert(0) += bytes;
    }

    pub fn elapsed_seconds(&self) -> f64 {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start).as_secs_f64(),
            (Some(start), None) => Instant::now().duration_since(start).as_secs_f64(),
            _ => 0.0,
        }
    }

    pub fn requests_per_second(&self) -> f64 {
        let elapsed = self.elapsed_seconds();
        if elapsed > 0.0 {
            self.requests_count as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Final result of a spider crawl.
pub struct CrawlResult {
    pub stats: CrawlStats,
    pub items: ItemList,
    pub paused: bool,
}

impl CrawlResult {
    pub fn completed(&self) -> bool {
        !self.paused
    }
}
```

- [ ] **Step 9: Create placeholder files for remaining spider modules**

Create `src/spiders/spider.rs`:

```rust
// Implemented in Task 10
```

Create `src/spiders/engine.rs`:

```rust
// Implemented in Task 11
```

Create `src/spiders/response.rs`:

```rust
use crate::fetchers::response::Response as FetcherResponse;
use crate::parser::Selector;

/// Spider response wrapping the fetcher response with additional spider context.
pub struct SpiderResponse {
    inner: FetcherResponse,
}

impl SpiderResponse {
    pub fn new(inner: FetcherResponse) -> Self {
        Self { inner }
    }

    pub fn status(&self) -> u16 {
        self.inner.status()
    }

    pub fn text(&self) -> &str {
        self.inner.text()
    }

    pub fn url(&self) -> &str {
        self.inner.url()
    }

    pub fn selector(&self) -> Selector {
        self.inner.selector()
    }

    pub fn css(&self, selector: &str) -> crate::parser::Selectors {
        self.selector().css(selector)
    }

    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        self.inner.json()
    }

    pub fn is_blocked(&self) -> bool {
        self.inner.is_blocked()
    }

    pub fn content_length(&self) -> usize {
        self.inner.content_length()
    }
}
```

Create `src/spiders/session.rs`:

```rust
// Implemented in Task 10
```

Create `src/spiders/checkpoint.rs`:

```rust
// Placeholder - implemented in Task 11
```

Create `src/spiders/cache.rs`:

```rust
// Placeholder - implemented in Task 11
```

Create `src/spiders/robots.rs`:

```rust
// Placeholder - implemented in Task 11
```

- [ ] **Step 10: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: implement spider Request, Scheduler, and Result types"
```

---

## Task 10: Spiders - Spider Trait & Session Manager

**Files:**
- Modify: `src/spiders/spider.rs`
- Modify: `src/spiders/session.rs`

- [ ] **Step 1: Implement the Spider trait**

Replace `src/spiders/spider.rs`:

```rust
use crate::fetchers::config::FetcherConfig;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::result::CrawlResult;
use async_trait::async_trait;
use std::collections::HashSet;

/// The core Spider trait that users implement to define a crawler.
/// Mirrors Python Scrapling's Spider ABC.
#[async_trait]
pub trait Spider: Send + Sync + 'static {
    /// Spider identifier. Must be unique.
    fn name(&self) -> &str;

    /// Seed URLs to start crawling from.
    fn start_urls(&self) -> Vec<String>;

    /// Domain whitelist. Empty = allow all.
    fn allowed_domains(&self) -> HashSet<String> {
        HashSet::new()
    }

    /// Whether to obey robots.txt.
    fn robots_txt_obey(&self) -> bool {
        false
    }

    /// Global concurrency limit.
    fn concurrent_requests(&self) -> u32 {
        4
    }

    /// Per-domain concurrency limit. 0 = disabled.
    fn concurrent_requests_per_domain(&self) -> u32 {
        0
    }

    /// Seconds to wait between requests.
    fn download_delay(&self) -> f64 {
        0.0
    }

    /// Max retries for blocked responses.
    fn max_blocked_retries(&self) -> u32 {
        3
    }

    /// Whether to include kwargs in request fingerprints.
    fn fp_include_kwargs(&self) -> bool {
        false
    }

    /// Whether to keep URL fragments in fingerprints.
    fn fp_keep_fragments(&self) -> bool {
        false
    }

    /// Whether to include headers in fingerprints.
    fn fp_include_headers(&self) -> bool {
        false
    }

    /// Enable dev-mode response caching.
    fn development_mode(&self) -> bool {
        false
    }

    /// Generate initial requests from start_urls.
    fn start_requests(&self) -> Vec<SpiderRequest> {
        self.start_urls()
            .into_iter()
            .map(|url| SpiderRequest::new(&url))
            .collect()
    }

    /// Default callback for processing responses.
    /// Returns a list of items (JSON values) and follow-up requests.
    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>);

    /// Hook called before crawling starts.
    async fn on_start(&self, _resuming: bool) {}

    /// Hook called after crawling ends.
    async fn on_close(&self) {}

    /// Hook called when a request fails.
    async fn on_error(&self, _request: &SpiderRequest, _error: &str) {}

    /// Item pipeline hook. Return None to drop the item.
    async fn on_scraped_item(
        &self,
        item: serde_json::Value,
    ) -> Option<serde_json::Value> {
        Some(item)
    }

    /// Check if a response indicates a blocked request.
    async fn is_blocked(&self, response: &SpiderResponse) -> bool {
        response.is_blocked()
    }

    /// Configure fetcher sessions. Override to customize.
    fn fetcher_config(&self) -> FetcherConfig {
        FetcherConfig::default()
    }
}
```

- [ ] **Step 2: Implement SessionManager**

Replace `src/spiders/session.rs`:

```rust
use crate::fetchers::client::Fetcher;
use crate::fetchers::config::FetcherConfig;
use crate::fetchers::response::Response;
use crate::spiders::request::SpiderRequest;
use std::collections::HashMap;

/// Manages named HTTP sessions for spider crawling.
pub struct SessionManager {
    sessions: HashMap<String, Fetcher>,
    default_config: FetcherConfig,
}

impl SessionManager {
    pub fn new(default_config: FetcherConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            default_config,
        }
    }

    /// Add a named session with a specific config.
    pub fn add_session(&mut self, name: &str, config: FetcherConfig) {
        self.sessions.insert(name.to_string(), Fetcher::new(config));
    }

    /// Ensure a default session exists.
    pub fn ensure_default(&mut self) {
        if !self.sessions.contains_key("default") {
            self.sessions.insert(
                "default".to_string(),
                Fetcher::new(self.default_config.clone()),
            );
        }
    }

    /// Fetch a URL using the specified session (or default).
    pub async fn fetch(&self, request: &SpiderRequest) -> Result<Response, String> {
        let session_id = if request.session_id().is_empty() {
            "default"
        } else {
            request.session_id()
        };

        let fetcher = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        match request.method() {
            "GET" => fetcher.get(request.url()).await.map_err(|e| e.to_string()),
            "POST" => fetcher
                .post(request.url(), request.body(), None)
                .await
                .map_err(|e| e.to_string()),
            "PUT" => fetcher
                .put(request.url(), request.body(), None)
                .await
                .map_err(|e| e.to_string()),
            "DELETE" => fetcher
                .delete(request.url())
                .await
                .map_err(|e| e.to_string()),
            m => Err(format!("Unsupported HTTP method: {}", m)),
        }
    }
}
```

- [ ] **Step 3: Add async-trait dependency**

In `Cargo.toml`, add:

```toml
async-trait = "0.1"
```

- [ ] **Step 4: Run tests to verify compilation**

Run: `cargo check`
Expected: Compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement Spider trait and SessionManager for crawl orchestration"
```

---

## Task 11: Spiders - Crawler Engine

**Files:**
- Modify: `src/spiders/engine.rs`
- Modify: `src/spiders/checkpoint.rs`
- Modify: `src/spiders/cache.rs`
- Modify: `src/spiders/robots.rs`
- Create: `tests/spiders/test_engine.rs`

- [ ] **Step 1: Implement robots.txt support**

Replace `src/spiders/robots.rs`:

```rust
use std::collections::HashMap;

/// Simple robots.txt manager that checks if a URL is allowed.
pub struct RobotsTxtManager {
    cache: HashMap<String, RobotsRules>,
    user_agent: String,
}

/// Parsed robots.txt rules for a domain.
struct RobotsRules {
    disallowed: Vec<String>,
    crawl_delay: Option<f64>,
}

impl RobotsTxtManager {
    pub fn new(user_agent: &str) -> Self {
        Self {
            cache: HashMap::new(),
            user_agent: user_agent.to_string(),
        }
    }

    /// Fetch and parse robots.txt for a domain.
    pub async fn fetch_robots(&mut self, domain: &str) {
        let url = format!("https://{}/robots.txt", domain);
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(text) = resp.text().await {
                    let rules = Self::parse_robots(&text, &self.user_agent);
                    self.cache.insert(domain.to_string(), rules);
                }
            }
            Err(_) => {
                // If we can't fetch, allow all
                self.cache.insert(
                    domain.to_string(),
                    RobotsRules {
                        disallowed: vec![],
                        crawl_delay: None,
                    },
                );
            }
        }
    }

    /// Check if a URL is allowed by robots.txt.
    pub fn is_allowed(&self, url: &str) -> bool {
        let domain = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()));

        let domain = match domain {
            Some(d) => d,
            None => return true,
        };

        let rules = match self.cache.get(&domain) {
            Some(r) => r,
            None => return true, // Not fetched yet = allow
        };

        let path = url::Url::parse(url)
            .ok()
            .map(|u| u.path().to_string())
            .unwrap_or_default();

        !rules.disallowed.iter().any(|d| path.starts_with(d))
    }

    /// Get the crawl delay for a domain.
    pub fn crawl_delay(&self, domain: &str) -> Option<f64> {
        self.cache.get(domain).and_then(|r| r.crawl_delay)
    }

    fn parse_robots(text: &str, user_agent: &str) -> RobotsRules {
        let mut disallowed = Vec::new();
        let mut crawl_delay = None;
        let mut active = false;
        let ua_lower = user_agent.to_lowercase();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some(ua) = line.strip_prefix("User-agent:").or_else(|| line.strip_prefix("user-agent:")) {
                let ua = ua.trim().to_lowercase();
                active = ua == "*" || ua_lower.contains(&ua);
            } else if active {
                if let Some(path) = line.strip_prefix("Disallow:").or_else(|| line.strip_prefix("disallow:")) {
                    let path = path.trim();
                    if !path.is_empty() {
                        disallowed.push(path.to_string());
                    }
                } else if let Some(delay) = line.strip_prefix("Crawl-delay:").or_else(|| line.strip_prefix("crawl-delay:")) {
                    if let Ok(d) = delay.trim().parse::<f64>() {
                        crawl_delay = Some(d);
                    }
                }
            }
        }

        RobotsRules {
            disallowed,
            crawl_delay,
        }
    }
}
```

- [ ] **Step 2: Implement dev-mode cache**

Replace `src/spiders/cache.rs`:

```rust
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Development-mode response cache.
/// Stores responses to disk keyed by URL fingerprint.
pub struct ResponseCache {
    cache_dir: PathBuf,
}

impl ResponseCache {
    pub fn new(cache_dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(cache_dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self { cache_dir: path })
    }

    /// Check if a cached response exists for the URL.
    pub fn get(&self, url: &str) -> Option<CachedResponse> {
        let path = self.cache_path(url);
        if path.exists() {
            let data = std::fs::read_to_string(&path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    /// Store a response in the cache.
    pub fn put(&self, url: &str, response: &CachedResponse) -> Result<(), std::io::Error> {
        let path = self.cache_path(url);
        let json = serde_json::to_string(response)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.cache_dir.join(format!("{}.json", &hash[..16]))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
}
```

- [ ] **Step 3: Implement checkpoint**

Replace `src/spiders/checkpoint.rs`:

```rust
use std::path::{Path, PathBuf};

/// Checkpoint manager for pause/resume support.
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CheckpointData {
    pub pending_urls: Vec<String>,
    pub seen_fingerprints: Vec<String>,
    pub items_count: u64,
}

impl CheckpointManager {
    pub fn new(dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            checkpoint_dir: path,
        })
    }

    pub fn checkpoint_path(&self) -> PathBuf {
        self.checkpoint_dir.join("checkpoint.json")
    }

    /// Save a checkpoint.
    pub fn save(&self, data: &CheckpointData) -> Result<(), std::io::Error> {
        let json = serde_json::to_string(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(self.checkpoint_path(), json)
    }

    /// Restore from checkpoint, if one exists.
    pub fn restore(&self) -> Option<CheckpointData> {
        let path = self.checkpoint_path();
        if path.exists() {
            let data = std::fs::read_to_string(&path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    /// Remove checkpoint file after successful completion.
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(self.checkpoint_path());
    }

    pub fn exists(&self) -> bool {
        self.checkpoint_path().exists()
    }
}
```

- [ ] **Step 4: Implement CrawlerEngine**

Replace `src/spiders/engine.rs`:

```rust
use crate::spiders::cache::{CachedResponse, ResponseCache};
use crate::spiders::checkpoint::{CheckpointData, CheckpointManager};
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::result::{CrawlResult, CrawlStats, ItemList};
use crate::spiders::robots::RobotsTxtManager;
use crate::spiders::scheduler::Scheduler;
use crate::spiders::session::SessionManager;
use crate::spiders::spider::Spider;
use crate::fetchers::response::Response;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, Semaphore};

/// The async crawl orchestrator. Created by Spider::start().
pub struct CrawlerEngine<S: Spider> {
    spider: Arc<S>,
    session_manager: Arc<SessionManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    stats: Arc<Mutex<CrawlStats>>,
    items: Arc<Mutex<ItemList>>,
    global_limiter: Arc<Semaphore>,
    robots_manager: Option<Arc<Mutex<RobotsTxtManager>>>,
    cache: Option<Arc<ResponseCache>>,
    checkpoint: Option<Arc<CheckpointManager>>,
    paused: Arc<std::sync::atomic::AtomicBool>,
    active_tasks: Arc<std::sync::atomic::AtomicU32>,
}

impl<S: Spider> CrawlerEngine<S> {
    pub fn new(
        spider: Arc<S>,
        session_manager: SessionManager,
        crawl_dir: Option<&str>,
    ) -> Self {
        let scheduler = Scheduler::new(
            spider.fp_include_kwargs(),
            spider.fp_include_headers(),
            spider.fp_keep_fragments(),
        );

        let robots = if spider.robots_txt_obey() {
            Some(Arc::new(Mutex::new(RobotsTxtManager::new("RUSTScrapling"))))
        } else {
            None
        };

        let cache = if spider.development_mode() {
            let dir = format!(".scrapling_cache/{}", spider.name());
            ResponseCache::new(&dir).ok().map(Arc::new)
        } else {
            None
        };

        let checkpoint = crawl_dir.map(|dir| {
            Arc::new(CheckpointManager::new(dir).expect("Failed to create checkpoint dir"))
        });

        Self {
            spider,
            session_manager: Arc::new(session_manager),
            scheduler: Arc::new(Mutex::new(scheduler)),
            stats: Arc::new(Mutex::new(CrawlStats::default())),
            items: Arc::new(Mutex::new(ItemList::new())),
            global_limiter: Arc::new(Semaphore::new(4)), // will be set properly
            robots_manager: robots,
            cache,
            checkpoint,
            paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            active_tasks: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Run the crawl. Main entry point.
    pub async fn crawl(self) -> CrawlResult {
        let global_limit = self.spider.concurrent_requests() as usize;
        let global_limiter = Arc::new(Semaphore::new(global_limit));

        // Set stats config
        {
            let mut stats = self.stats.lock().await;
            stats.concurrent_requests = self.spider.concurrent_requests();
            stats.concurrent_requests_per_domain = self.spider.concurrent_requests_per_domain();
            stats.download_delay = self.spider.download_delay();
            stats.start_time = Some(Instant::now());
        }

        // Check for checkpoint restore
        let resuming = if let Some(ref cp) = self.checkpoint {
            if let Some(data) = cp.restore() {
                let mut sched = self.scheduler.lock().await;
                for url in &data.pending_urls {
                    sched.enqueue(SpiderRequest::new(url));
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        // Call on_start
        self.spider.on_start(resuming).await;

        // Pre-fetch robots.txt
        if let Some(ref robots) = self.robots_manager {
            let domains: Vec<String> = self
                .spider
                .start_urls()
                .iter()
                .filter_map(|u| url::Url::parse(u).ok())
                .filter_map(|u| u.host_str().map(|s| s.to_string()))
                .collect();
            let mut rm = robots.lock().await;
            for domain in domains {
                rm.fetch_robots(&domain).await;
            }
        }

        // Enqueue start requests (unless resuming)
        if !resuming {
            let requests = self.spider.start_requests();
            let mut sched = self.scheduler.lock().await;
            for req in requests {
                sched.enqueue(req);
            }
        }

        // Main crawl loop
        loop {
            if self
                .paused
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                break;
            }

            let request = {
                let mut sched = self.scheduler.lock().await;
                sched.dequeue()
            };

            match request {
                Some(req) => {
                    let permit = global_limiter.clone().acquire_owned().await.unwrap();
                    let spider = self.spider.clone();
                    let session_manager = self.session_manager.clone();
                    let scheduler = self.scheduler.clone();
                    let stats = self.stats.clone();
                    let items = self.items.clone();
                    let robots = self.robots_manager.clone();
                    let cache = self.cache.clone();
                    let active = self.active_tasks.clone();

                    active.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    tokio::spawn(async move {
                        let _permit = permit;
                        Self::process_request(
                            spider,
                            session_manager,
                            scheduler,
                            stats,
                            items,
                            robots,
                            cache,
                            req,
                        )
                        .await;
                        active.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    });
                }
                None => {
                    // Check if all tasks are done
                    if self
                        .active_tasks
                        .load(std::sync::atomic::Ordering::Relaxed)
                        == 0
                    {
                        // Double-check scheduler is still empty
                        let sched = self.scheduler.lock().await;
                        if sched.is_empty() {
                            break;
                        }
                    }
                    // Wait a bit before checking again
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }

        // Finalize
        {
            let mut stats = self.stats.lock().await;
            stats.end_time = Some(Instant::now());
        }

        self.spider.on_close().await;

        let paused = self.paused.load(std::sync::atomic::Ordering::Relaxed);

        // Save checkpoint if paused, cleanup if completed
        if let Some(ref cp) = self.checkpoint {
            if paused {
                let sched = self.scheduler.lock().await;
                let data = CheckpointData {
                    pending_urls: sched
                        .pending_requests()
                        .iter()
                        .map(|r| r.url().to_string())
                        .collect(),
                    seen_fingerprints: vec![],
                    items_count: self.items.lock().await.len() as u64,
                };
                let _ = cp.save(&data);
            } else {
                cp.cleanup();
            }
        }

        let stats = self.stats.lock().await.clone();
        let items = std::mem::take(&mut *self.items.lock().await);

        CrawlResult {
            stats,
            items,
            paused,
        }
    }

    async fn process_request(
        spider: Arc<S>,
        session_manager: Arc<SessionManager>,
        scheduler: Arc<Mutex<Scheduler>>,
        stats: Arc<Mutex<CrawlStats>>,
        items: Arc<Mutex<ItemList>>,
        robots: Option<Arc<Mutex<RobotsTxtManager>>>,
        cache: Option<Arc<ResponseCache>>,
        request: SpiderRequest,
    ) {
        // Check robots.txt
        if let Some(ref rm) = robots {
            let rm = rm.lock().await;
            if !rm.is_allowed(request.url()) {
                stats.lock().await.robots_disallowed_count += 1;
                return;
            }
        }

        // Check allowed domains
        let allowed = spider.allowed_domains();
        if !allowed.is_empty() {
            let domain = request.domain();
            let is_allowed = allowed.iter().any(|d| domain.ends_with(d.as_str()));
            if !is_allowed {
                stats.lock().await.offsite_requests_count += 1;
                return;
            }
        }

        // Check dev-mode cache
        if let Some(ref c) = cache {
            if let Some(cached) = c.get(request.url()) {
                stats.lock().await.cache_hits += 1;
                let response = Response::new(
                    cached.status,
                    cached.content_type,
                    cached.body,
                    cached.url,
                    cached.headers,
                );
                let spider_response = SpiderResponse::new(response);
                Self::run_callbacks(spider, scheduler, stats, items, &request, spider_response)
                    .await;
                return;
            } else {
                stats.lock().await.cache_misses += 1;
            }
        }

        // Apply download delay
        let delay = spider.download_delay();
        if delay > 0.0 {
            tokio::time::sleep(std::time::Duration::from_secs_f64(delay)).await;
        }

        // Fetch
        stats.lock().await.increment_requests_count();
        match session_manager.fetch(&request).await {
            Ok(response) => {
                let bytes = response.content_length() as u64;
                let domain = request.domain();
                {
                    let mut s = stats.lock().await;
                    s.increment_status(response.status());
                    s.increment_response_bytes(bytes, &domain);
                }

                // Cache response in dev mode
                if let Some(ref c) = cache {
                    let _ = c.put(
                        request.url(),
                        &CachedResponse {
                            status: response.status(),
                            content_type: response.content_type().to_string(),
                            body: response.text().to_string(),
                            url: response.url().to_string(),
                            headers: response.headers().clone(),
                        },
                    );
                }

                let spider_response = SpiderResponse::new(response);

                // Check if blocked
                if spider.is_blocked(&spider_response).await {
                    stats.lock().await.blocked_requests_count += 1;
                    if request.retry_count() < spider.max_blocked_retries() {
                        let mut retry = request.copy();
                        retry.increment_retry();
                        retry.set_dont_filter(true);
                        scheduler.lock().await.enqueue(retry);
                    }
                    return;
                }

                Self::run_callbacks(spider, scheduler, stats, items, &request, spider_response)
                    .await;
            }
            Err(e) => {
                stats.lock().await.failed_requests_count += 1;
                spider.on_error(&request, &e).await;
            }
        }
    }

    async fn run_callbacks(
        spider: Arc<S>,
        scheduler: Arc<Mutex<Scheduler>>,
        stats: Arc<Mutex<CrawlStats>>,
        items: Arc<Mutex<ItemList>>,
        request: &SpiderRequest,
        response: SpiderResponse,
    ) {
        let (scraped_items, follow_requests) = spider.parse(response).await;

        // Process items
        for item in scraped_items {
            if let Some(processed) = spider.on_scraped_item(item).await {
                items.lock().await.push(processed);
                stats.lock().await.items_scraped += 1;
            } else {
                stats.lock().await.items_dropped += 1;
            }
        }

        // Enqueue follow-up requests
        let mut sched = scheduler.lock().await;
        for req in follow_requests {
            sched.enqueue(req);
        }
    }

    /// Request the engine to pause.
    pub fn request_pause(&self) {
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
```

- [ ] **Step 5: Write an integration test**

Create `tests/spiders/test_engine.rs`:

```rust
use rust_scrapling::spiders::spider::Spider;
use rust_scrapling::spiders::request::SpiderRequest;
use rust_scrapling::spiders::response::SpiderResponse;
use rust_scrapling::spiders::engine::CrawlerEngine;
use rust_scrapling::spiders::session::SessionManager;
use rust_scrapling::fetchers::config::FetcherConfig;
use async_trait::async_trait;
use std::sync::Arc;

struct TestSpider;

#[async_trait]
impl Spider for TestSpider {
    fn name(&self) -> &str {
        "test"
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://httpbin.org/html".to_string()]
    }

    fn concurrent_requests(&self) -> u32 {
        1
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let sel = response.selector();
        let h1 = sel.css("h1");
        let title = if !h1.is_empty() {
            h1[0].text().as_str().to_string()
        } else {
            "unknown".to_string()
        };
        let item = serde_json::json!({
            "title": title,
            "url": response.url(),
        });
        (vec![item], vec![])
    }
}

#[tokio::test]
async fn test_crawler_engine_basic() {
    let spider = Arc::new(TestSpider);
    let mut session_manager = SessionManager::new(FetcherConfig::default());
    session_manager.ensure_default();

    let engine = CrawlerEngine::new(spider, session_manager, None);
    let result = engine.crawl().await;

    assert!(result.completed());
    assert!(result.items.len() >= 1);
    assert!(result.stats.requests_count >= 1);
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: implement CrawlerEngine with robots.txt, caching, and checkpointing"
```

---

## Task 12: Public API & CLI

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Set up the public API in lib.rs**

Replace `src/lib.rs`:

```rust
//! RUSTScrapling - A Rust port of the Scrapling web scraping framework.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use rust_scrapling::parser::Selector;
//!
//! let html = "<html><body><h1>Hello</h1></body></html>";
//! let sel = Selector::from_html(html);
//! let h1 = sel.css("h1");
//! println!("{}", h1[0].text());
//! ```

pub mod core;
pub mod parser;
pub mod fetchers;
pub mod spiders;

// Re-export primary types at crate root for convenience
pub use parser::{Selector, Selectors};
pub use fetchers::client::Fetcher;
pub use fetchers::config::FetcherConfig;
pub use fetchers::response::Response;
pub use spiders::spider::Spider;
pub use spiders::request::SpiderRequest;
pub use spiders::result::{CrawlResult, CrawlStats, ItemList};
pub use spiders::engine::CrawlerEngine;
```

- [ ] **Step 2: Implement the CLI**

Replace `src/main.rs`:

```rust
use clap::{Parser, Subcommand};
use rust_scrapling::fetchers::client::Fetcher;
use rust_scrapling::fetchers::config::FetcherConfig;

#[derive(Parser)]
#[command(name = "rust-scrapling")]
#[command(about = "RUSTScrapling - A Rust web scraping framework", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch a URL and extract content
    Fetch {
        /// The URL to fetch
        url: String,

        /// CSS selector to extract
        #[arg(short, long)]
        selector: Option<String>,

        /// Output format: text, html, json
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Disable stealth headers
        #[arg(long)]
        no_stealth: bool,
    },

    /// Extract text content from a URL
    Extract {
        /// The URL to extract from
        url: String,

        /// CSS selector to target
        #[arg(short, long)]
        selector: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch {
            url,
            selector,
            format,
            no_stealth,
        } => {
            let config = FetcherConfig::builder()
                .stealth(!no_stealth)
                .build();
            let fetcher = Fetcher::new(config);

            match fetcher.get(&url).await {
                Ok(response) => {
                    if let Some(css) = selector {
                        let sel = response.selector();
                        let results = sel.css(&css);
                        for item in &results {
                            match format.as_str() {
                                "html" => println!("{}", item.outer_html()),
                                "json" => {
                                    let obj = serde_json::json!({
                                        "tag": item.tag(),
                                        "text": item.text().as_str(),
                                        "html": item.html_content().as_str(),
                                    });
                                    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
                                }
                                _ => println!("{}", item.text()),
                            }
                        }
                        eprintln!("Found {} elements", results.len());
                    } else {
                        match format.as_str() {
                            "html" => println!("{}", response.text()),
                            _ => {
                                let sel = response.selector();
                                println!(
                                    "{}",
                                    sel.get_all_text("\n", true, &["script", "style"], true)
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Extract { url, selector } => {
            let fetcher = Fetcher::new(FetcherConfig::default());

            match fetcher.get(&url).await {
                Ok(response) => {
                    let sel = response.selector();
                    if let Some(css) = selector {
                        let results = sel.css(&css);
                        for item in &results {
                            println!("{}", item.text());
                        }
                    } else {
                        println!(
                            "{}",
                            sel.get_all_text("\n", true, &["script", "style"], true)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify everything compiles and tests pass**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: add public API re-exports and CLI with fetch/extract commands"
```

---

## Task 13: Final Integration Test & Polish

**Files:**
- Create: `tests/integration_test.rs`

- [ ] **Step 1: Write a comprehensive integration test**

Create `tests/integration_test.rs`:

```rust
use rust_scrapling::parser::Selector;
use rust_scrapling::core::{TextHandler, AttributesHandler};

const ECOMMERCE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head><title>Shop - Products</title></head>
<body>
<div id="products" class="product-list">
  <div class="product" data-id="1">
    <h2 class="product-name">Laptop Pro</h2>
    <span class="price">$999.99</span>
    <p class="description">A powerful laptop for professionals.</p>
    <a href="/products/1" class="details-link">View Details</a>
  </div>
  <div class="product" data-id="2">
    <h2 class="product-name">Wireless Mouse</h2>
    <span class="price">$29.99</span>
    <p class="description">Ergonomic wireless mouse.</p>
    <a href="/products/2" class="details-link">View Details</a>
  </div>
  <div class="product" data-id="3">
    <h2 class="product-name">USB-C Hub</h2>
    <span class="price">$49.99</span>
    <p class="description">7-in-1 USB-C hub with HDMI.</p>
    <a href="/products/3" class="details-link">View Details</a>
  </div>
</div>
<nav>
  <a href="/page/2" class="next-page">Next</a>
</nav>
<script>var analytics = {};</script>
</body>
</html>
"#;

#[test]
fn test_full_scraping_workflow() {
    let sel = Selector::from_html_with_url(ECOMMERCE_HTML, "https://shop.example.com/page/1");

    // Extract all product names
    let names = sel.css("h2.product-name");
    assert_eq!(names.len(), 3);
    assert_eq!(names[0].text().as_str(), "Laptop Pro");
    assert_eq!(names[1].text().as_str(), "Wireless Mouse");
    assert_eq!(names[2].text().as_str(), "USB-C Hub");

    // Extract prices using regex
    let prices = sel.css("span.price");
    let all_prices: Vec<TextHandler> = prices
        .iter()
        .flat_map(|p| p.re(r"\$(\d+\.\d+)", true, false, true))
        .collect();
    assert_eq!(all_prices.len(), 3);
    assert_eq!(all_prices[0].as_str(), "999.99");

    // Extract product links
    let links = sel.css("a.details-link");
    for link in &links {
        let href = link.attrib().get("href").unwrap().clone();
        let abs_url = sel.urljoin(href.as_str());
        assert!(abs_url.starts_with("https://shop.example.com/products/"));
    }

    // Navigate DOM - find product by data-id
    let products = sel.css("[data-id]");
    assert_eq!(products.len(), 3);
    let second = products.filter(|p| {
        p.attrib()
            .get("data-id")
            .map(|v| v.as_str() == "2")
            .unwrap_or(false)
    });
    assert_eq!(second.len(), 1);
    let name = second[0].css("h2.product-name");
    assert_eq!(name[0].text().as_str(), "Wireless Mouse");

    // Find pagination link
    let next = sel.css("a.next-page");
    assert_eq!(next.len(), 1);
    let next_url = sel.urljoin(next[0].attrib().get("href").unwrap().as_str());
    assert_eq!(next_url, "https://shop.example.com/page/2");

    // Text extraction ignoring script tags
    let body_text = sel.get_all_text(" ", true, &["script", "style"], true);
    assert!(!body_text.as_str().contains("analytics"));
    assert!(body_text.as_str().contains("Laptop Pro"));

    // Find by text
    let found = sel.find_by_text("Wireless Mouse", true, false, false);
    assert!(found.is_some());
    assert_eq!(found.unwrap().tag(), "h2");
}

#[test]
fn test_text_handler_chaining() {
    let t = TextHandler::new("  Hello World  ");
    let result = t.strip().to_lowercase().replace_str("world", "rust");
    assert_eq!(result.as_str(), "hello rust");
}

#[test]
fn test_complex_selectors() {
    let html = r#"
    <html><body>
    <div class="a"><div class="b"><span>Deep</span></div></div>
    <div class="a"><span>Shallow</span></div>
    </body></html>
    "#;
    let sel = Selector::from_html(html);

    // Descendant selector
    let deep = sel.css("div.a div.b span");
    assert_eq!(deep.len(), 1);
    assert_eq!(deep[0].text().as_str(), "Deep");

    // Direct child selector
    let shallow = sel.css("div.a > span");
    assert_eq!(shallow.len(), 1);
    assert_eq!(shallow[0].text().as_str(), "Shallow");
}
```

- [ ] **Step 2: Run the full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 3: Run clippy for code quality**

Run: `cargo clippy -- -W clippy::all`
Expected: No errors (warnings acceptable)

- [ ] **Step 4: Verify the CLI builds**

Run: `cargo build --release`
Expected: Binary builds successfully

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add integration tests and finalize RUSTScrapling v0.1.0"
```

---

## Summary

| Task | Component | Key Types |
|------|-----------|-----------|
| 1 | Project Setup | Cargo.toml, module structure |
| 2 | TextHandler | `TextHandler`, `TextHandlers` |
| 3 | AttributesHandler | `AttributesHandler` |
| 4 | Parser | `Selector`, `Selectors` |
| 5 | Storage | `SqliteStorage` |
| 6 | Selector Generation | `generate_css_selector`, `generate_xpath_selector`, `CssQuery` |
| 7 | Fetcher Config | `FetcherConfig`, `ProxyRotator`, constants |
| 8 | HTTP Client | `Fetcher`, `Response` |
| 9 | Spider Core | `SpiderRequest`, `Scheduler`, `CrawlStats`, `ItemList` |
| 10 | Spider Trait | `Spider` trait, `SessionManager` |
| 11 | Crawler Engine | `CrawlerEngine`, `RobotsTxtManager`, `ResponseCache`, `CheckpointManager` |
| 12 | Public API & CLI | `lib.rs` re-exports, CLI commands |
| 13 | Integration Tests | End-to-end verification |
