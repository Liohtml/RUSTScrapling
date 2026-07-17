<div align="center">

# RUSTScrapling

![RUSTScrapling — the crab that weaves the web](https://raw.githubusercontent.com/Liohtml/RUSTScrapling/master/docs/assets/demo.gif)

**A high-performance Rust port of [Scrapling](https://github.com/D4Vinci/Scrapling) — the modern web scraping framework built by web scrapers, for web scrapers.**

[![CI](https://github.com/Liohtml/RUSTScrapling/actions/workflows/ci.yml/badge.svg)](https://github.com/Liohtml/RUSTScrapling/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

*Parse HTML with CSS selectors, survive page redesigns with adaptive selectors, and crawl entire sites with async concurrency — from a single Rust crate.*

</div>

---

## What is this?

Web scraping in three sentences:

1. **You point it at HTML** — a string, a URL, or a whole site — and query it with plain CSS selectors (`page.css(".price")`).
2. **When a site redesigns and your selector breaks**, adaptive mode finds the element again by similarity to what it looked like before — no code change needed.
3. **When one page isn't enough**, the spider engine crawls whole sites concurrently with rate limiting, deduplication, robots.txt compliance, and pause/resume checkpoints.

It's a faithful Rust port of Python's [Scrapling](https://github.com/D4Vinci/Scrapling) (same concepts, same API names), which means: **~5× faster extraction, ~4× less memory, and a single native binary** — no interpreter, no virtualenv, no cold start. [Benchmarks below.](#how-fast-is-it)

| Layer | What it gives you | Key types |
|-------|-------------------|-----------|
| **Parser** | CSS selectors, DOM navigation, regex extraction, adaptive relocation | `Selector`, `Selectors` |
| **Fetchers** | Async HTTP with retries, stealth headers, charset-aware decoding, proxy rotation | `Fetcher`, `Response` |
| **Spiders** | Concurrent crawling: rules, sitemaps, dedup, checkpoints | `Spider`, `CrawlSpider`, `SitemapSpider`, `CrawlerEngine` |
| **Core** | Rich text values, attribute maps, SQLite element storage | `TextHandler`, `SqliteStorage` |

Each layer works on its own — use just the parser, or the whole stack.

---

## Table of Contents

- [Quick Start](#quick-start)
- [How fast is it?](#how-fast-is-it)
- [Using it in AI agent workflows](#using-it-in-ai-agent-workflows)
- [Usage Guide](#usage-guide)
  - [Parsing HTML](#parsing-html)
  - [CSS Selectors](#css-selectors)
  - [Adaptive Selectors (survive redesigns)](#adaptive-selectors-survive-redesigns)
  - [Text Extraction](#text-extraction)
  - [DOM Navigation](#dom-navigation)
  - [Regex Extraction](#regex-extraction)
  - [Fetching Pages](#fetching-pages)
  - [Building a Spider](#building-a-spider)
  - [CrawlSpider, SitemapSpider & LinkExtractor](#crawlspider-sitemapspider--linkextractor)
- [CLI](#cli)
- [Architecture](#architecture)
- [Testing](#testing)
- [Contributing](#contributing)
- [License](#license)
- [Credits](#credits)

---

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rust_scrapling = "0.2"
tokio = { version = "1", features = ["full"] }
serde_json = "1"          # items are serde_json::Value
async-trait = "0.1"       # only needed when implementing the Spider trait
```

Or track the latest development version straight from git:

```toml
rust_scrapling = { git = "https://github.com/Liohtml/RUSTScrapling" }
```

**Requirements:** Rust 1.88+ (edition 2021)

Thirty seconds to your first scrape:

```rust
use rust_scrapling::{Selector, Fetcher, FetcherConfig};

#[tokio::main]
async fn main() {
    // 1. Parse any HTML string
    let page = Selector::from_html(r#"<h1 class="title">Hello World</h1>"#);
    println!("{}", page.css("h1.title")[0].text()); // "Hello World"

    // 2. Or fetch a live page (stealth headers on by default)
    let fetcher = Fetcher::new(FetcherConfig::default()).unwrap();
    let response = fetcher.get("https://example.com").await.unwrap();
    for link in response.selector().css("a") {
        println!("{} -> {:?}", link.text(), link.get_attribute("href"));
    }
}
```

Crawl a whole site from its sitemap in one builder chain:

```rust
use rust_scrapling::{CrawlerEngine, SitemapSpider, CrawlRule, LinkExtractor, FetcherConfig};
use rust_scrapling::spiders::session::SessionManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spider = SitemapSpider::builder("shop")
        .sitemap_urls(["https://shop.example.com/robots.txt"]) // finds sitemaps automatically
        .rule(CrawlRule::new(LinkExtractor::new().allow([r"/products/"])?))
        .parse_item(Arc::new(|resp| {
            resp.css("h1.product-title")
                .into_iter()
                .map(|t| serde_json::json!({ "title": t.text().to_string(), "url": resp.url() }))
                .collect()
        }))
        .build();

    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        None,
    )?;
    let result = engine.crawl().await;
    result.items.to_jsonl(std::path::Path::new("products.jsonl"))?;
    Ok(())
}
```

---

## How fast is it?

Measured head-to-head against the original Python Scrapling (v0.4.10, lxml-backed) on the **same machine, same document, same selectors, same extraction work**: a 616 KB e-commerce page with 1,000 product cards; each run parses the page and extracts title, price, and link from every card. Median of 30 runs, single-threaded.

| Workload | Python Scrapling 0.4.10 | RUSTScrapling (release) | Speedup |
|----------|------------------------:|------------------------:|:-------:|
| Parse + extract 3,000 fields from 1,000 products | 105.7 ms | **19.2 ms** | **~5.5×** |
| Parse only (build the DOM) | 15.6 ms | 15.7 ms | ~1× |
| Peak process memory for the same job¹ | 37 MB | **9 MB** | **~4×** |

<sub>¹ Peak RSS of a single parse+extract run: Python via `resource.getrusage(RUSAGE_SELF).ru_maxrss`, Rust via `VmHWM` in `/proc/self/status`. The timing scripts below don't measure memory.</sub>

What the numbers mean, honestly:

- **Raw HTML parsing is a tie** — lxml's C parser is excellent, and html5ever matches it. The Rust win is everything *after* parsing: selector matching, DOM traversal, and text extraction have no interpreter overhead, so end-to-end extraction is ~5.5× faster.
- **Memory scales the same way**: ~9 MB peak vs ~37 MB for one page. Crawling with 8 concurrent workers, that gap compounds.
- **Concurrency is where Rust pulls furthest ahead in practice**: the spider engine runs on `tokio` — thousands of in-flight requests on a few OS threads, no GIL, no multiprocessing serialization overhead when fanning work out.
- **Deployment**: `cargo build --release` produces one native binary. No Python runtime, no dependency resolution at deploy time, millisecond cold starts — which matters for serverless scrapers and agent tools (next section).

Reproduce it yourself — the fixture generator and both benchmark scripts are in the repo:

```bash
# from the repo root:
python3 scripts/benchmark/gen_page.py      # writes the shared page.html fixture
python3 scripts/benchmark/bench_python.py  # pip install scrapling first
cargo run --release --example benchmark
```

---

## Using it in AI agent workflows

Scraping is one of the most common tools an LLM agent calls — and one of the most punishing: it runs on every task, latency lands inside the agent loop, and a broken selector silently poisons downstream reasoning. RUSTScrapling is a good fit for exactly those reasons:

- **Latency lives in your agent loop.** 19 ms instead of 106 ms per page extraction matters when an agent chains fetch → extract → reason several times per task.
- **Adaptive selectors reduce silent breakage.** When a target site redesigns, `css_adaptive` relocates the element by similarity instead of returning nothing — your agent's tool keeps working instead of feeding it empty results.
- **Deterministic, structured output.** Selectors + JSON out, no LLM tokens spent parsing HTML. Feed the model *data*, not markup.
- **One native binary.** Trivially shippable as a sandboxed tool next to your agent runtime — no Python environment in the container.

### Pattern 1 — expose it as a tool (function calling / MCP)

Wrap fetch + extract in one function with a JSON contract, then register it as a tool in your agent framework (Anthropic tool use, an MCP server, LangChain, etc.):

```rust
use rust_scrapling::{Fetcher, FetcherConfig};

/// Tool: fetch a URL and extract elements by CSS selector.
/// Input:  { "url": "...", "selector": "..." }
/// Output: [ { "tag": "...", "text": "...", "attrs": {...} } ]
pub async fn scrape_tool(url: &str, selector: &str) -> anyhow::Result<serde_json::Value> {
    let fetcher = Fetcher::new(FetcherConfig::default())?;
    let response = fetcher.get(url).await?;
    let items: Vec<serde_json::Value> = response
        .selector()
        .css(selector)
        .into_iter()
        .map(|el| {
            let attrs: serde_json::Map<String, serde_json::Value> = el
                .attrib()
                .iter()
                .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
                .collect();
            serde_json::json!({
                "tag": el.tag(),
                "text": el.get_all_text(" ", true, &["script", "style"], None).to_string(),
                "attrs": attrs,
            })
        })
        .collect();
    Ok(serde_json::json!(items))
}
```

A matching tool definition for the model:

```json
{
  "name": "scrape",
  "description": "Fetch a web page and extract elements matching a CSS selector. Returns structured JSON.",
  "input_schema": {
    "type": "object",
    "properties": {
      "url": { "type": "string" },
      "selector": { "type": "string", "description": "CSS selector, e.g. 'h1' or '.price'" }
    },
    "required": ["url", "selector"]
  }
}
```

### Pattern 2 — the CLI as a zero-code agent tool

Agents that can run shell commands can use RUSTScrapling with no integration work at all — the CLI emits JSON:

```bash
rust-scrapling fetch https://example.com --selector "h1" --format json
```

Allow-list that one binary in your agent's sandbox and you have a fast, memory-safe scraping tool with no network stack inside the agent process itself.

### Pattern 3 — self-healing extraction for long-running agents

Autonomous agents monitoring sites for weeks can't page a human when a selector dies. Save the element once, then let relocation absorb redesigns:

```rust
use rust_scrapling::core::storage::SqliteStorage;
use rust_scrapling::parser::DEFAULT_RELOCATION_PERCENTAGE;

let store = SqliteStorage::new("elements.db", "https://example.com")?;

// css_adaptive = normal CSS first; if the selector stops matching,
// find the element again by similarity to its saved snapshot.
let prices = page.css_adaptive(
    "#main .price",   // the selector that might break
    "product-price",  // stable identifier for this element
    &store,
    true,             // auto_save: keep the snapshot fresh as the page drifts
    DEFAULT_RELOCATION_PERCENTAGE,
)?;
```

The agent's extraction tool now degrades gracefully: redesign → relocation → same JSON output, and the refreshed snapshot tracks the new page structure.

### Pattern 4 — bulk collection for RAG pipelines

Use `SitemapSpider` to turn a whole site into clean JSONL for chunking and embedding — the quick-start example above is exactly that: robots.txt → sitemap index → filtered URLs → structured items, with dedup, rate limiting, and resumable checkpoints for large corpora built in.

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

let items = page.css("div.product");           // by class
let main = page.css("#main-content");          // by ID
let priced = page.css("[data-price]");         // by attribute
let links = page.css("nav > ul > li > a.active"); // compound
```

The result is a `Selectors` collection with batch operations:

```rust
let items = page.css("li.item");

let first = &items[0];
for item in &items {
    println!("{}: {}", item.tag(), item.text());
}

let special = items.filter(|item| item.has_class("special"));
let target = items.search(|item| item.text().as_str() == "Target");
let names = page.css("div.product").css("h2.name");  // chain queries
let all_text: Vec<_> = items.getall();
```

### Adaptive Selectors (survive redesigns)

Save an element's "fingerprint" (tag, attributes, text, position, parent/sibling context) to SQLite. When the selector later stops matching — because the site changed — `relocate`/`css_adaptive` scores every element in the new page against the snapshot and returns the best match above a similarity threshold:

```rust
use rust_scrapling::core::storage::SqliteStorage;
use rust_scrapling::parser::{Selector, DEFAULT_RELOCATION_PERCENTAGE};

let store = SqliteStorage::new("elements.db", "https://example.com")?;

// First run: selector works, element gets saved under "product-card".
let page = Selector::from_html(&old_html);
page.css_adaptive("#products .product", "product-card", &store, true, DEFAULT_RELOCATION_PERCENTAGE)?;

// After a redesign: same call, selector fails, relocation finds the moved element.
let page = Selector::from_html(&new_html);
let found = page.css_adaptive("#products .product", "product-card", &store, true, DEFAULT_RELOCATION_PERCENTAGE)?;
assert!(!found.is_empty());
```

The similarity scoring is a port of upstream Scrapling's algorithm (with a difflib-parity sequence matcher), so behavior matches the Python original.

### Text Extraction

`TextHandler` wraps every text value with regex, JSON, and cleaning methods:

```rust
let text = element.text();                         // direct text
let all = element.get_all_text("\n", true, &["script", "style"], None); // recursive
let cleaned = element.text().strip().to_lowercase().replace_str("old", "new");
let data = element.text().json()?;                 // parse as JSON
let inner = element.html_content();
let outer = element.outer_html();
```

### DOM Navigation

```rust
let products = page.css("li.product");
let item = products.first().unwrap();

let list = item.parent().unwrap();
let children = list.children();
let siblings = item.siblings();
let next = item.next();
let prev = item.previous();

let attrs = item.attrib();
let id = attrs.get("data-id").unwrap();

let heading = page.find_by_text("Hello World", true, false, false);
let matched = page.find_by_regex(r"Item \d+", true, false);
```

### Regex Extraction

```rust
let price_els = page.css("span.price");
let price_el = price_els.first().unwrap();

let prices = price_el.re(r"\$(\d+\.\d+)", true, false, true);        // all matches
let first = price_el.re_first(r"\$(\d+\.\d+)", true, false, true);   // first match
let all = page.css("span.price").re(r"\$(\d+\.\d+)", true, false, true); // batch
```

### Fetching Pages

The `Fetcher` is an async HTTP client with retries, stealth headers, charset-aware body decoding, and proxy support:

```rust
use rust_scrapling::{Fetcher, FetcherConfig};

// Default config: 30s timeout, 3 retries, stealth headers on
let fetcher = Fetcher::new(FetcherConfig::default())?;

// Custom config via builder
let fetcher = Fetcher::new(
    FetcherConfig::builder()
        .timeout(60)
        .retries(5)
        .proxy("http://proxy:8080")
        .user_agent("MyBot/1.0")
        .stealth(true)
        .header("Authorization", "Bearer token123")
        .build()
)?;

let resp = fetcher.get("https://example.com").await?;
let resp = fetcher.post("https://api.example.com/data", Some(body), None).await?;

let page = resp.selector();       // Response -> Selector (auto-parses HTML)
println!("Status: {}", resp.status());
println!("Blocked: {}", resp.is_blocked());
```

Response bodies are decoded according to the `Content-Type` charset (ISO-8859-1, Shift_JIS, windows-1252, …) — no mojibake from non-UTF-8 sites.

#### Search engines (DuckDuckGo)

DuckDuckGo's HTML endpoint rate-limits automated requests aggressively and on detection returns **HTTP 202 with its homepage** instead of results. It also wraps result links in a redirect. The `fetchers::search` helpers handle both:

```rust
use rust_scrapling::fetchers::search::{decode_duckduckgo_href, is_duckduckgo_blocked};

let resp = fetcher.get("https://html.duckduckgo.com/html/?q=rust").await?;
if is_duckduckgo_blocked(&resp) {
    eprintln!("DuckDuckGo soft-blocked this request");
} else {
    for link in resp.selector().css("a.result__a") {
        let raw = link.attrib().get("href").map(|h| h.as_str().to_string()).unwrap_or_default();
        println!("{}", decode_duckduckgo_href(&raw));
    }
}
```

### Building a Spider

Implement the `Spider` trait for full control over a crawl:

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

    async fn parse(&self, response: SpiderResponse)
        -> (Vec<serde_json::Value>, Vec<SpiderRequest>)
    {
        let page = response.selector();
        let items = page.css("div.product").into_iter().map(|p| serde_json::json!({
            "name":  p.css("h2.name").first().map(|n| n.text().to_string()),
            "price": p.css("span.price").first().map(|n| n.text().to_string()),
        })).collect();

        let requests = page.css("a.next-page").into_iter()
            .filter_map(|l| l.get_attribute("href"))
            .map(|href| SpiderRequest::new(&page.urljoin(href.as_str())))
            .collect();

        (items, requests)
    }
}

#[tokio::main]
async fn main() {
    let engine = CrawlerEngine::new(
        Arc::new(ProductSpider),
        SessionManager::new(FetcherConfig::default()),
        None,
    ).unwrap();
    let result = engine.crawl().await;
    println!("Scraped {} items", result.items.len());
    result.items.to_jsonl(std::path::Path::new("products.jsonl")).unwrap();
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
| `on_scraped_item(item)` | Item pipeline — return `None` to drop |
| `is_blocked(response)` | Custom block detection |

> **Memory note:** the scheduler keeps a dedup set of one fingerprint per unique
> URL visited (~100 MB per ~1M URLs), persisted in checkpoints so paused crawls
> resume without re-visiting pages. For very large or open-ended crawls, set
> `allowed_domains()` to bound scope.

### CrawlSpider, SitemapSpider & LinkExtractor

For the common cases you don't need to implement `Spider` yourself:

**`LinkExtractor`** — declarative URL discovery with regex allow/deny, domain filters (subdomains included), CSS scoping, and a built-in binary-extension deny list (including compound extensions like `.tar.gz`):

```rust
use rust_scrapling::LinkExtractor;

let extractor = LinkExtractor::new()
    .allow([r"/articles/"]).unwrap()
    .deny([r"/tag/"]).unwrap()
    .allow_domains(["example.com"])       // matches api.example.com too
    .restrict_css(["#content"]);          // only look inside #content

let urls: Vec<String> = extractor.extract(&response); // absolute, deduped, canonicalized
```

**`CrawlSpider`** — follow links matching rules, extract items from every page:

```rust
use rust_scrapling::{CrawlSpider, CrawlRule, LinkExtractor};
use std::sync::Arc;

let spider = CrawlSpider::builder("articles")
    .start_urls(["https://example.com/"])
    .rule(CrawlRule::new(LinkExtractor::new().allow([r"/articles/"]).unwrap()).priority(5))
    .parse_item(Arc::new(|resp| {
        resp.css("h1").into_iter()
            .map(|h| serde_json::json!({ "title": h.text().to_string() }))
            .collect()
    }))
    .build();
```

**`SitemapSpider`** — seed a crawl from sitemaps (or robots.txt `Sitemap:` directives), recurse through sitemap indexes, and dispatch page URLs through rules — see the [Quick Start](#quick-start) for a complete example.

**`ShopifySpider`** — extract every product variant from any Shopify-powered store through its JSON API (`collections.json` → per-collection `products.json`), no HTML parsing and no selectors to maintain:

```rust
use rust_scrapling::ShopifySpider;

let spider = ShopifySpider::builder("shop.example.com").build();
// items: { name, price, category, brand, identifier, sku, stock,
//          image_url, url, description, old_price, barcode }
```

---

## CLI

RUSTScrapling includes a command-line tool for quick scraping:

```bash
# Fetch a page and extract text
rust-scrapling fetch https://example.com

# Extract specific elements with a CSS selector
rust-scrapling fetch https://example.com --selector "h1"

# Output as HTML or JSON (tag, text, html per element)
rust-scrapling fetch https://example.com --selector "div.content" --format html
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
|   |-- adaptive.rs                # Adaptive relocation: save/retrieve/relocate
|   |-- selector_generation.rs     # Auto-generate CSS/XPath from DOM position
|   +-- translator.rs              # ::text and ::attr() pseudo-elements
|
|-- fetchers/                      # HTTP layer
|   |-- client.rs                  # Fetcher: async HTTP with retries
|   |-- config.rs                  # FetcherConfig: builder pattern
|   |-- encoding.rs                # Charset-aware response decoding
|   |-- response.rs                # Response: auto-parses to Selector
|   |-- proxy.rs                   # ProxyRotator: round-robin proxy cycling
|   |-- search.rs                  # DuckDuckGo helpers (block detection, URL decode)
|   +-- constants.rs               # User agents, status codes, headers
|
+-- spiders/                       # Crawl framework
    |-- spider.rs                  # Spider trait (user-facing API)
    |-- engine.rs                  # CrawlerEngine: async orchestrator
    |-- links.rs                   # LinkExtractor: URL discovery primitive
    |-- templates/                 # CrawlSpider + CrawlRule, SitemapSpider
    |-- request.rs                 # SpiderRequest: fingerprinting + priority
    |-- response.rs                # SpiderResponse: parser integration
    |-- result.rs                  # CrawlResult, CrawlStats, ItemList
    |-- scheduler.rs               # Priority queue with deduplication
    |-- session.rs                 # SessionManager: named HTTP sessions
    |-- robots.rs                  # robots.txt compliance
    |-- cache.rs                   # Dev-mode response caching
    +-- checkpoint.rs              # Pause/resume persistence (incl. dedup state)
```

### Design Principles

- **Each layer is independent.** Use just the parser without fetchers. Use fetchers without spiders. Compose as needed.
- **Cheap tree sharing.** `Selector` uses `Rc<Html>` to share the parsed tree — child selectors point into the same tree instead of copying it.
- **Async-first.** The fetcher and spider layers are built on `tokio` for high-concurrency crawling.
- **Scrapy-compatible API names.** `css()`, `text()`, `re()`, `re_first()`, `get()`, `getall()` mirror Scrapy/Parsel conventions.

---

## Testing

```bash
cargo test                     # 275+ tests, 3 network tests ignored
cargo test -- --ignored        # include network tests
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

CI runs the full suite on Linux, macOS, and Windows, plus clippy, rustfmt, MSRV (1.88), and `cargo audit` on every push.

---

## Contributing

Contributions are welcome! Fork, branch, write tests, run `cargo test && cargo clippy`, and open a PR.

### Areas for Contribution

- **Browser automation** — headless Chrome/Playwright integration (like Python Scrapling's `StealthyFetcher`/`DynamicFetcher`)
- **MCP server** — a ready-made Model Context Protocol wrapper around the fetch/extract API
- **Interactive shell** — REPL for exploring pages
- **Performance** — more benchmarks, SIMD text processing, zero-copy parsing
- **Documentation** — more examples, tutorials, API docs

---

## License

Licensed under either of:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

---

## Credits

- **[Scrapling](https://github.com/D4Vinci/Scrapling)** by [Karim Shoair](https://github.com/D4Vinci) — the original Python framework that inspired this project
- **[scraper](https://github.com/causal-agent/scraper)** — HTML parsing and CSS selection in Rust
- **[reqwest](https://github.com/seanmonstar/reqwest)** — HTTP client
- **[tokio](https://tokio.rs/)** — Async runtime
- Banner art generated with [Z-Image-Turbo](https://huggingface.co/spaces/Tongyi-MAI/Z-Image-Turbo) on Hugging Face Spaces

---

<div align="center">

**Built with Rust. Inspired by Scrapling. Made for scraping.**

</div>
