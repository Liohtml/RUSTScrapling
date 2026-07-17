# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-07-17

First release shipped through the automated release pipeline (tag,
GitHub release, and crates.io publish are created by CI on version bump).

### Added

- `documentation` link (docs.rs) in the crate metadata, shown on the crates.io page

## [0.2.0] - 2026-07-17

First release published to crates.io. Syncs all applicable changes from
upstream Scrapling v0.4.8 through v0.4.11.

### Added

- **Adaptive element relocation**: `Selector::save`/`retrieve`/`relocate` and `css_adaptive` — persist an element's fingerprint to SQLite and find it again by similarity after the page structure changes. Difflib-parity sequence matcher (position index + autojunk), default threshold 40%
- **`LinkExtractor`**: declarative URL discovery with regex allow/deny, domain filters (subdomain-aware), `restrict_css` scoping, URL canonicalization, and a binary-extension deny list including compound extensions (`.tar.gz`)
- **`CrawlSpider` + `CrawlRule`**: follow links matching declarative rules, with priority override and `process_request` hook
- **`SitemapSpider`**: seed crawls from sitemaps or robots.txt `Sitemap:` directives, recurse through sitemap indexes (filtered by `sitemap_follow`), dispatch URLs through rules
- **`ShopifySpider`**: extract every product variant from any Shopify store via its JSON API (`collections.json` → `products.json`), no HTML parsing (upstream v0.4.11)
- **Benchmarks**: reproducible parse+extract comparison vs Python Scrapling (`scripts/benchmark/`, `examples/benchmark.rs`)

### Changed

- **Charset-aware response decoding**: bodies are decoded per the `Content-Type` charset via `encoding_rs` (quoted values, whitespace tolerance, replacement-encoding fallback) instead of always lossy UTF-8
- **Checkpoints persist the dedup set**: resumed crawls no longer re-visit URLs crawled before a pause; checkpoints are written compactly
- README rewritten: animated hero, plain-language intro, benchmark results, AI-agent integration patterns

### Fixed

- `LinkExtractor` extracts in true document order across tags; empty queries normalize away; non-UTF-8 percent-escapes survive canonicalization
- Sitemap parsing is robust to html5ever's re-nesting of self-closing elements; relative sitemap URLs resolve against the response URL; content pages mentioning `<urlset` in scripts are no longer misclassified

## [0.1.0] - 2026-05-04

### Added

- **Core types**: `TextHandler` (string wrapper with regex, JSON, cleaning), `TextHandlers` (batch ops), `AttributesHandler` (read-only attribute map)
- **SQLite storage**: Persistent element tracking for adaptive mode via `SqliteStorage`
- **HTML parser**: `Selector` with CSS selector support, text extraction, DOM navigation (parent, children, siblings, next, previous)
- **Selector generation**: Auto-generate CSS/XPath selectors from element position
- **CSS translator**: Support for `::text` and `::attr()` pseudo-elements
- **HTTP client**: Async `Fetcher` with configurable retries, stealth headers, proxy support
- **Fetcher config**: Builder pattern with timeout, retries, proxy, user agent, stealth headers
- **Proxy rotation**: Round-robin and random proxy selection
- **Response integration**: `Response` auto-parses to `Selector` for immediate HTML querying
- **Spider trait**: User-facing API with configurable concurrency, rate limiting, domain filtering
- **Crawler engine**: Async orchestrator with `tokio`, semaphore-based concurrency, robots.txt compliance
- **Request fingerprinting**: SHA-256 based deduplication in priority scheduler
- **Dev-mode caching**: Disk-based response cache for development iteration
- **Checkpoint system**: Pause/resume support for long-running crawls
- **robots.txt**: Automatic compliance with Disallow rules and Crawl-delay
- **CLI**: `fetch` and `extract` subcommands with CSS selector and format options
- **175 tests** covering all modules

[0.2.1]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.1
[0.2.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.0
[0.1.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.1.0
