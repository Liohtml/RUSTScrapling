<div align="center">

# RUSTScrapling

**A high-performance Rust port of [Scrapling](https://github.com/D4Vinci/Scrapling) -- the modern web scraping framework built by web scrapers, for web scrapers.**

[![CI](https://github.com/Liohtml/RUSTScrapling/actions/workflows/ci.yml/badge.svg)](https://github.com/Liohtml/RUSTScrapling/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

*Parse HTML with CSS selectors, fetch pages with stealth headers, and crawl entire sites with async concurrency -- all from a single Rust crate.*

</div>

---

## Why RUSTScrapling?

The original [Scrapling](https://github.com/D4Vinci/Scrapling) (Python) combines three powerful ideas in one framework:

1. **Adaptive Parsing** -- CSS/XPath selectors that can relocate elements when page structure changes
2. **Multi-Strategy Fetching** -- simple HTTP, stealth-mode, and browser automation in one API
3. **Spider-Based Crawling** -- Scrapy-inspired async crawlers with rate limiting, deduplication, and checkpointing

**RUSTScrapling** brings this to Rust with native performance, memory safety, and zero-cost abstractions. It's structured as four independent layers that compose together:

| Layer | Purpose | Key Types |
|-------|---------|-----------|
| **Core** | Rich string types, attribute maps, persistent storage | `TextHandler`, `AttributesHandler`, `SqliteStorage` |
| **Parser** | HTML parsing with CSS selectors, DOM traversal, regex | `Selector`, `Selectors` |
| **Fetchers** | Async HTTP with retries, stealth headers, proxy rotation | `Fetcher`, `FetcherConfig`, `Response` |
| **Spiders** | Concurrent crawl orchestration | `Spider` trait, `CrawlerEngine`, `SpiderRequest` |

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Usage Guide](#usage-guide)
  - [Parsing HTML](#parsing-html)
  - [CSS Selectors](#css-selectors)
  - [Text Extraction](#text-extraction)
  - [DOM Navigation](#dom-navigation)
  - [Regex Extraction](#regex-extraction)
  - [Fetching Pages](#fetching-pages)
  - [Building a Spider](#building-a-spider)
- [CLI](#cli)
- [Architecture](#architecture)
- [API Reference](#api-reference)
- [Testing](#testing)
- [Contributing](#contributing)
- [License](#license)
- [Credits](#credits)

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rust_scrapling = { git = "https://github.com/Liohtml/RUSTScrapling" }
```

Or clone and build locally:

```bash
git clone https://github.com/Liohtml/RUSTScrapling.git
cd RUSTScrapling
cargo build --release
```

**Requirements:** Rust 1.88+ (edition 2021)

---

## Quick Start

```rust
use rust_scrapling::{Selector, Fetcher, FetcherConfig};

// -- Parse static HTML --
let html = r#"<html><body>
  <h1 class="title">Hello World</h1>
  <a href="/about">About</a>
</body></html>"#;

let page = Selector::from_html(html);
let title = page.css("h1.title");
println!("{}", title[0].text());  // "Hello World"

// -- Fetch a live page --
#[tokio::main]
async fn main() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher.get("https://example.com").await.unwrap();
    let page = response.selector();

    for link in page.css("a") {
        let href = link.attrib().get("href");
        println!("{}: {}", link.text(), href.map(|h| h.as_str()).unwrap_or(""));
    }
}
```

---

## Usage Guide

### Parsing HTML

Create a `Selector` from any HTML string. It wraps the parsed DOM tree and provides the full query API.

```rust
use rust_scrapling::Selector;

let page = Selector::from_html("<html><body><p>Hello</p></body></html>");

// With a base URL (enables urljoin for relative links)
let page = Selector::from_html_with_url(html, "https://example.com/page/1");
let absolute = page.urljoin("/about");  // "https://example.com/about"
```

### CSS Selectors

Full CSS3 selector support powered by the `scraper` crate:

```rust
let page = Selector::from_html(html);

// By class
let items = page.css("div.product");

// By ID
let main = page.css("#main-content");

// By attribute
let priced = page.css("[data-price]");

// Compound selectors
let links = page.css("nav > ul > li > a.active");

// Descendant selectors
let deep = page.css("div.container span.highlight");
```

The result is a `Selectors` collection with batch operations:

```rust
let items = page.css("li.item");

// Access by index
let first = &items[0];
let last = items.last().unwrap();

// Iterate
for item in &items {
    println!("{}: {}", item.tag(), item.text());
}

// Filter
let special = items.filter(|item| item.has_class("special"));

// Search (find first match)
let target = items.search(|item| item.text().as_str() == "Target");

// Chain CSS queries
let names = page.css("div.product").css("h2.name");

// Batch text extraction
let all_text: Vec<_> = items.getall();  // Vec<TextHandler>
```

### Text Extraction

`TextHandler` wraps every text value with regex, JSON, and cleaning methods:

```rust
// Direct text (immediate children only)
let text = element.text();  // TextHandler

// All text recursively, ignoring <script> and <style>
let all_text = element.get_all_text("\n", true, &["script", "style"], None);

// Chaining
let cleaned = element.text().strip().to_lowercase().replace_str("old", "new");

// JSON parsing
let data = element.text().json().unwrap();  // serde_json::Value

// Inner/outer HTML
let inner = element.html_content();
let outer = element.outer_html();
```

### DOM Navigation

```rust
let item = page.css("li.product").first().unwrap();

// Parent
let list = item.parent().unwrap();
assert_eq!(list.tag(), "ul");

// Children (element nodes only)
let children = list.children();

// Siblings
let siblings = item.siblings();
let next = item.next();
let prev = item.previous();

// Attributes
let attrs = item.attrib();
let id = attrs.get("data-id").unwrap();
let has_class = item.has_class("featured");

// Search by text
let heading = page.find_by_text("Hello World", true, false, false);
let partial = page.find_by_text("Hello", true, true, false);  // partial match

// Search by regex
let match_ = page.find_by_regex(r"Item \d+", true, false);
```

### Regex Extraction

Extract data from text using regex, with capture group support:

```rust
let price_el = page.css("span.price").first().unwrap();

// All matches (returns capture group 1 if present, else group 0)
let prices = price_el.re(r"\$(\d+\.\d+)", true, false, true);
// prices[0].as_str() == "19.99"

// First match only
let first = price_el.re_first(r"\$(\d+\.\d+)", true, false, true);

// Batch regex across multiple elements
let all_prices = page.css("span.price").re(r"\$(\d+\.\d+)", true, false, true);
```

### Fetching Pages

The `Fetcher` is an async HTTP client with retries, stealth headers, and proxy support:

```rust
use rust_scrapling::{Fetcher, FetcherConfig};

// Default config: 30s timeout, 3 retries, stealth headers on
let fetcher = Fetcher::new(FetcherConfig::default());

// Custom config via builder
let fetcher = Fetcher::new(
    FetcherConfig::builder()
        .timeout(60)
        .retries(5)
        .proxy("http://proxy:8080")
        .user_agent("MyBot/1.0")
        .stealth(true)
        .verify_ssl(false)
        .header("Authorization", "Bearer token123")
        .build()
);

// HTTP methods
let resp = fetcher.get("https://example.com").await?;
let resp = fetcher.post("https://api.example.com/data", Some(body), None).await?;
let resp = fetcher.put("https://api.example.com/data/1", None, Some(&json_val)).await?;
let resp = fetcher.delete("https://api.example.com/data/1").await?;

// Response -> Selector (auto-parses HTML)
let page = resp.selector();
let titles = page.css("h1");

// Response metadata
println!("Status: {}", resp.status());
println!("URL: {}", resp.url());
println!("Blocked: {}", resp.is_blocked());
let json_data = resp.json()?;  // Parse as JSON
```

#### Search engines (DuckDuckGo)

DuckDuckGo's HTML endpoint (`html.duckduckgo.com/html/`) rate-limits automated
requests aggressively — even with stealth headers — and on detection returns
**HTTP 202 with its homepage** instead of results. Because 202 is nominally a
success code, `resp.is_blocked()` does not catch it. It also wraps every result
link in a `//duckduckgo.com/l/?uddg=…` redirect. The `fetchers::search` helpers
handle both:

```rust
use rust_scrapling::fetchers::search::{decode_duckduckgo_href, is_duckduckgo_blocked};

let resp = fetcher.get("https://html.duckduckgo.com/html/?q=rust").await?;
if is_duckduckgo_blocked(&resp) {
    eprintln!("DuckDuckGo soft-blocked this request (HTTP 202 homepage)");
} else {
    for link in resp.selector().css("a.result__a") {
        let raw = link.attrib().get("href").map(|h| h.as_str().to_string()).unwrap_or_default();
        println!("{}", decode_duckduckgo_href(&raw)); // real target, not the /l/ redirect
    }
}
```

For reliable automated search, prefer **Startpage** or the **DuckDuckGo Instant
Answers API** (`api.duckduckgo.com/?format=json`).

### Building a Spider

Define a spider by implementing the `Spider` trait:

```rust
use rust_scrapling::{Spider, SpiderRequest, CrawlerEngine, FetcherConfig};
use rust_scrapling::spiders::response::SpiderResponse;
use rust_scrapling::spiders::session::SessionManager;
use async_trait::async_trait;
use std::sync::Arc;

struct ProductSpider;

#[async_trait]
impl Spider for ProductSpider {
    fn name(&self) -> &str { "products" }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://shop.example.com/products".into()]
    }

    fn concurrent_requests(&self) -> u32 { 8 }
    fn download_delay(&self) -> f64 { 0.5 }
    fn robots_txt_obey(&self) -> bool { true }

    fn allowed_domains(&self) -> std::collections::HashSet<String> {
        ["shop.example.com".into()].into()
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let page = response.selector();
        let mut items = Vec::new();
        let mut requests = Vec::new();

        // Extract product data
        for product in page.css("div.product") {
            let name = product.css("h2.name");
            let price = product.css("span.price");

            items.push(serde_json::json!({
                "name": name.first().map(|n| n.text().as_str().to_string()),
                "price": price.first().map(|p| p.text().as_str().to_string()),
                "url": response.url(),
            }));
        }

        // Follow pagination
        for link in page.css("a.next-page") {
            if let Some(href) = link.attrib().get("href") {
                let next_url = page.urljoin(href.as_str());
                requests.push(SpiderRequest::new(&next_url));
            }
        }

        (items, requests)
    }

    async fn on_scraped_item(&self, item: serde_json::Value) -> Option<serde_json::Value> {
        // Filter out items without a price
        if item.get("price").is_some() { Some(item) } else { None }
    }
}

#[tokio::main]
async fn main() {
    let spider = Arc::new(ProductSpider);
    let mut session_manager = SessionManager::new(FetcherConfig::default());
    session_manager.ensure_default();

    let engine = CrawlerEngine::new(spider, session_manager, None);
    let result = engine.crawl().await;

    println!("Scraped {} items in {:.1}s",
        result.items.len(),
        result.stats.elapsed_seconds());
    println!("Requests: {}, Failed: {}",
        result.stats.requests_count,
        result.stats.failed_requests_count);

    // Export results
    result.items.to_json("products.json", true).unwrap();
    result.items.to_jsonl("products.jsonl").unwrap();
}
```

#### Spider Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `concurrent_requests()` | `4` | Global concurrency limit |
| `concurrent_requests_per_domain()` | `0` | Per-domain limit (0 = disabled) |
| `download_delay()` | `0.0` | Seconds between requests |
| `robots_txt_obey()` | `false` | Respect robots.txt |
| `max_blocked_retries()` | `3` | Retry limit for blocked responses |
| `allowed_domains()` | `{}` | Domain whitelist (empty = allow all) |
| `development_mode()` | `false` | Cache responses to disk for dev iteration |

#### Spider Lifecycle Hooks

| Hook | When |
|------|------|
| `on_start(resuming)` | Before crawl begins |
| `on_close()` | After crawl ends |
| `on_error(request, error)` | When a request fails |
| `on_scraped_item(item)` | Item pipeline -- return `None` to drop |
| `is_blocked(response)` | Custom block detection |

> **Memory note:** the scheduler keeps an in-memory dedup set of one
> fingerprint per unique URL visited (~100 MB per ~1M URLs). For very large
> or open-ended crawls, set `allowed_domains()` to bound scope so the set does
> not grow without limit.

---

## CLI

RUSTScrapling includes a command-line tool for quick scraping:

```bash
# Fetch a page and extract text
rust-scrapling fetch https://example.com

# Extract specific elements with a CSS selector
rust-scrapling fetch https://example.com --selector "h1"

# Output as HTML
rust-scrapling fetch https://example.com --selector "div.content" --format html

# Output as JSON (tag, text, html per element)
rust-scrapling fetch https://example.com --selector "a" --format json

# Disable stealth headers
rust-scrapling fetch https://example.com --no-stealth

# Extract text content (shorthand)
rust-scrapling extract https://example.com --selector "p"
```

---

## Architecture

```
rust_scrapling/
|
|-- core/                          # Foundation types
|   |-- text_handler.rs            # TextHandler: String + regex/json/clean
|   |-- text_handlers.rs           # TextHandlers: Vec<TextHandler> batch ops
|   |-- attributes_handler.rs      # AttributesHandler: read-only attr map
|   +-- storage.rs                 # SqliteStorage: adaptive element persistence
|
|-- parser/                        # HTML parsing engine
|   |-- selector.rs                # Selector: element wrapper (CSS, text, nav)
|   |-- selectors.rs               # Selectors: batch operations
|   |-- selector_generation.rs     # Auto-generate CSS/XPath from DOM position
|   +-- translator.rs              # ::text and ::attr() pseudo-elements
|
|-- fetchers/                      # HTTP layer
|   |-- client.rs                  # Fetcher: async HTTP with retries
|   |-- config.rs                  # FetcherConfig: builder pattern
|   |-- response.rs                # Response: auto-parses to Selector
|   |-- proxy.rs                   # ProxyRotator: round-robin proxy cycling
|   +-- constants.rs               # User agents, status codes, headers
|
+-- spiders/                       # Crawl framework
    |-- spider.rs                  # Spider trait (user-facing API)
    |-- engine.rs                  # CrawlerEngine: async orchestrator
    |-- request.rs                 # SpiderRequest: fingerprinting + priority
    |-- response.rs                # SpiderResponse: parser integration
    |-- result.rs                  # CrawlResult, CrawlStats, ItemList
    |-- scheduler.rs               # Priority queue with deduplication
    |-- session.rs                 # SessionManager: named HTTP sessions
    |-- robots.rs                  # robots.txt compliance
    |-- cache.rs                   # Dev-mode response caching
    +-- checkpoint.rs              # Pause/resume persistence
```

### Design Principles

- **Each layer is independent.** Use just the parser without fetchers. Use fetchers without spiders. Compose as needed.
- **Zero hidden allocations.** `Selector` uses `Rc<Html>` to share the parsed tree. Child selectors point into the same tree.
- **Async-first.** The fetcher and spider layers are built on `tokio` for high-concurrency crawling.
- **Scrapy-compatible API names.** `css()`, `text()`, `re()`, `re_first()`, `get()`, `getall()` mirror Scrapy/Parsel conventions.

---

## API Reference

### Core Types

| Type | Description |
|------|-------------|
| `TextHandler` | String wrapper with `.re()`, `.json()`, `.clean()`, `.strip()`, `.replace_str()` |
| `TextHandlers` | `Vec<TextHandler>` with batch `.re()`, `.re_first()` |
| `AttributesHandler` | Read-only attribute map with `.get()`, `.search_values()`, `.json_string()` |
| `SqliteStorage` | SQLite-backed element storage for adaptive mode |

### Parser Types

| Type | Description |
|------|-------------|
| `Selector` | HTML element wrapper -- `.css()`, `.text()`, `.attrib()`, `.children()`, `.parent()`, `.find_by_text()` |
| `Selectors` | Element collection -- `.css()`, `.filter()`, `.search()`, `.getall()`, `.re()` |

### Fetcher Types

| Type | Description |
|------|-------------|
| `Fetcher` | Async HTTP client -- `.get()`, `.post()`, `.put()`, `.delete()` |
| `FetcherConfig` | Config builder -- `.timeout()`, `.retries()`, `.proxy()`, `.stealth()` |
| `Response` | HTTP response -- `.selector()`, `.json()`, `.status()`, `.is_blocked()` |
| `ProxyRotator` | Round-robin proxy rotation |

### Spider Types

| Type | Description |
|------|-------------|
| `Spider` (trait) | User implements `.parse()`, configures `start_urls`, concurrency, etc. |
| `CrawlerEngine<S>` | Async orchestrator -- `.crawl()` returns `CrawlResult` |
| `SpiderRequest` | Request with fingerprinting, priority, metadata |
| `CrawlResult` | Final result -- `.items`, `.stats`, `.completed()` |
| `CrawlStats` | Metrics -- requests, bytes, items, timing, status codes |
| `ItemList` | Scraped items -- `.to_json()`, `.to_jsonl()` |

---

## Testing

```bash
# Run all tests (175 pass, 3 network tests ignored)
cargo test

# Run with network tests
cargo test -- --ignored

# Run a specific test module
cargo test parser_selector
cargo test core_text_handler
cargo test integration_test

# Run with logging
RUST_LOG=debug cargo test

# Check code quality
cargo clippy -- -W clippy::all

# Build release binary
cargo build --release
```

### Test Coverage

| Module | Tests | Coverage |
|--------|-------|----------|
| Core (TextHandler, AttributesHandler, Storage) | 64 | All public methods |
| Parser (Selector, Selectors, Generation) | 38 | CSS, text, nav, regex, DOM |
| Fetchers (Config, Client, Response) | 21 | Config, headers, Response struct |
| Spiders (Request, Scheduler, Result) | 27 | Fingerprinting, dedup, priority, export |
| Integration | 28 | End-to-end scraping workflows |
| **Total** | **178** | |

---

## Contributing

Contributions are welcome! Here's how to get started:

1. **Fork** the repository
2. **Create a branch** for your feature (`git checkout -b feature/amazing-feature`)
3. **Write tests** for your changes
4. **Run the test suite** (`cargo test && cargo clippy`)
5. **Commit** with a descriptive message
6. **Push** and open a Pull Request

### Development Setup

```bash
git clone https://github.com/Liohtml/RUSTScrapling.git
cd RUSTScrapling
cargo build
cargo test
```

### Areas for Contribution

- **Browser automation** -- Headless Chrome/Playwright integration (like Python Scrapling's `StealthyFetcher`/`DynamicFetcher`)
- **Adaptive mode** -- Element relocation using similarity scoring (storage layer is ready)
- **Interactive shell** -- REPL for exploring pages
- **Performance** -- Benchmarks, SIMD text processing, zero-copy parsing
- **Documentation** -- More examples, tutorials, API docs

---

## License

Licensed under either of:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

---

## Credits

- **[Scrapling](https://github.com/D4Vinci/Scrapling)** by [Karim Shoair](https://github.com/D4Vinci) -- the original Python framework that inspired this project
- **[scraper](https://github.com/causal-agent/scraper)** -- HTML parsing and CSS selection in Rust
- **[reqwest](https://github.com/seanmonstar/reqwest)** -- HTTP client
- **[tokio](https://tokio.rs/)** -- Async runtime

---

<div align="center">

**Built with Rust. Inspired by Scrapling. Made for scraping.**

</div>
