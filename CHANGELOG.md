# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/Liohtml/RUSTScrapling/releases/tag/v0.1.0
