# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-08-12

Typed errors throughout the fetch path — the one long-planned breaking
change. Migration is mechanical; details below.

### Changed (breaking)

- **`FetcherError` is now fully typed** (and `#[non_exhaustive]`):
  `RequestFailed(String)` is replaced by
  `ExhaustedRetries { attempts, source: reqwest::Error }`,
  `TooLarge { limit_bytes, advertised_bytes }`, and
  `BodyRead(reqwest::Error)`. Convenience classifiers
  `is_timeout()` / `is_connect()` / `transport_error()` support retry
  policies without digging into `reqwest`.
- **`SessionError::Network` carries the typed `FetcherError`** instead of
  a pre-rendered string (`Display` output is equivalent).
- **`Spider::on_error` receives `&SessionError`** instead of `&str`:
  hooks can now branch on failure class (timeout vs DNS vs body-size).
  Migration: change the signature; call `error.to_string()` to keep the
  old behavior.

### Added

- `Response::latency()`: wall-clock time of the successful fetch attempt
  (zero for cache replays / hand-built responses). AutoThrottle now
  adapts to this instead of a measurement that included the fetcher's
  internal transport retries and sleeps — flaky connections no longer
  over-throttle a healthy domain.

### Internal

- The engine's per-request state is bundled in one shared context (a
  single `Arc` clone per task instead of eight).

## [0.2.3] - 2026-08-12

Maintenance release: correctness quick-wins from a second-pass audit,
dependency majors, and repository process hardening.

### Changed

- `generate_css_selector` now emits `[id="…"]` (escaped) instead of a raw
  `#id`, so generated selectors are valid for ids containing `.`, `:`,
  spaces, or quotes — note the output-format change if you snapshot
  generated selectors
- `TextHandler::clean` collapses whitespace in a single pass (was O(n²)
  on pathological whitespace runs); semantics unchanged
- CLI `--format` is a clap `ValueEnum` (`text`|`html`|`json`): typos are
  rejected with the choices listed instead of silently meaning text
- Dependencies: rusqlite 0.40, sha2 0.11 (digest hex encoding is
  byte-identical, pinned by test — persisted fingerprints/cache keys are
  unaffected), clap 4.6.5, log 0.4.33, async-trait 0.1.91

### Fixed

- DuckDuckGo block detection compared URL substrings, so
  `evil.com/?ref=duckduckgo.com` or `duckduckgo.com.evil.com` were
  treated as DuckDuckGo; now an exact host/subdomain comparison
- A `Spider::download_delay()` returning a non-finite or absurdly large
  value could panic the dispatch loop; sanitized like the robots
  Crawl-delay floor

### Internal

- Property-based test verifies the robots.txt wildcard matcher against a
  regex oracle (50k cases locally, fresh seeds every CI run)
- CI: coverage report job (cargo-llvm-cov summary per run; baseline
  85% lines / 91% functions), build caches for clippy/MSRV, prebuilt
  cargo-audit (audit job now takes seconds)
- SECURITY.md (private reporting via GitHub Security Advisories), issue
  and PR templates; README documents AutoThrottle, CSV/XML export, and
  `::text`/`::attr()`

## [0.2.2] - 2026-08-05

Upstream v0.4.12 sync (AutoThrottle, CSV/XML export) plus the results of
a full internal audit: seven silent-corruption bug fixes, engine
concurrency fixes, complete robots.txt matching, `::text`/`::attr()`
support, and a complete rustdoc pass.

### Added

- **AutoThrottle** (upstream v0.4.12): adaptive per-domain crawl delays
  driven by measured response latency — 429/503 double the delay or
  honor `Retry-After`, healthy responses ease it back down;
  `download_delay` and robots.txt `Crawl-delay` are floors. Opt-in via
  `Spider::autothrottle_enabled()`; spacing uses a per-domain
  reservation clock so domains never delay each other
- **CSV and XML export** (upstream v0.4.12): `ItemList::to_csv` (RFC
  4180, header = union of keys across heterogeneous items, nested
  values as JSON strings) and `ItemList::to_xml` (entity-escaped,
  XML-invalid control characters stripped, element names sanitized)
- **`::text` and `::attr(name)` pseudo-elements**: Parsel-inspired
  `css_get`/`css_getall` on `Selector` and `SpiderResponse`
- **Complete robots.txt matching**: `Allow:` directives with
  longest-match-wins evaluation (Allow wins ties), `*`/`$` wildcards
  matched against path + query, rules scoped per scheme + authority
  (RFC 9309), combined same-agent groups, user-agent product-token
  matching, robots.txt fetched from the request's actual origin
  (scheme/host/port) instead of hardcoded `https://host/`
- Per-domain (`domains_response_bytes`) and per-session
  (`sessions_requests_count`) crawl statistics are now populated
- Complete rustdoc documentation: crate overview with examples, every
  public item documented (`missing_docs` enforced), `#[must_use]` on
  builders and query methods

### Changed

- **Engine concurrency**: the per-domain semaphore is acquired inside
  the spawned task instead of the dispatch loop, so one saturated
  domain no longer stalls dispatch for every other domain; the
  robots.txt network fetch no longer holds the manager lock (up to 10s)
  while other tasks wait to check `is_allowed`; dev-cache replays skip
  the download delay entirely
- `serde_json` now uses `preserve_order`: item keys keep insertion
  order (Python dict parity), so exports show fields in the order
  spiders build them. Note for consumers: Cargo feature unification
  enables insertion-ordered `serde_json::Map` crate-wide; and
  fingerprints of requests whose `meta` contains nested objects can
  differ from ones persisted by 0.2.1 (a resumed old checkpoint may
  re-fetch such URLs once)
- `tokio` dependency trimmed from `full` to the features actually used;
  `once_cell` replaced by `std::sync::LazyLock`; unused `http` and
  `tokio-test` dependencies removed

### Fixed

- **Per-request headers were never sent**: headers set via
  `SpiderRequestBuilder::header()` now reach the wire
- **Checkpoints preserved only URLs**: pending requests now persist in
  full (method, headers, body, meta, priority); old URL-only
  checkpoints still restore
- **Blocked responses were cached in dev mode**, so retries replayed
  the same blocked page from disk forever; the `is_blocked` check now
  runs before the cache write
- HTML entity decoding no longer double-decodes escaped entities
  (`&amp;lt;` now correctly yields the literal text `&lt;`)
- `find_by_text` matched different text depending on `first_match`;
  both paths now use recursive text with deepest-match-first descent
- `SpiderRequest`'s `Ord`/`PartialEq` contract violation fixed (tie
  break on fingerprint)
- `SqliteStorage::retrieve` no longer swallows real database errors as
  "not found"
- `ProxyRotator::random()` now advances its cursor
- Hostile robots.txt `Crawl-delay` values (`inf`, `NaN`, huge numbers)
  can no longer panic request tasks

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

[0.3.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.3.0
[0.2.3]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.3
[0.2.2]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.2
[0.2.1]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.1
[0.2.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.2.0
[0.1.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.1.0
