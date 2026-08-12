# Security Policy

## Supported Versions

Only the latest release on [crates.io](https://crates.io/crates/rust_scrapling)
receives security fixes.

| Version | Supported |
|---------|-----------|
| latest 0.x release | yes |
| older releases | no |

## Reporting a Vulnerability

Please report vulnerabilities privately via
[GitHub Security Advisories](https://github.com/Liohtml/RUSTScrapling/security/advisories/new)
— do **not** open a public issue for security problems.

You can expect an initial response within a week. Once a fix is released,
the advisory is published and credited unless you prefer otherwise.

## Scope notes

- This crate parses **hostile input by design** (arbitrary web pages,
  robots.txt files, HTTP headers). Panics, unbounded memory growth, or
  logic bypasses (e.g. of robots.txt or block detection) triggered by
  remote content are all in scope.
- SQLite is statically vendored via `rusqlite`'s `bundled` feature, so
  SQLite CVEs are addressed by dependency bumps here rather than OS
  updates. Dependabot and `cargo audit` (in CI) track these.
- The `verify_ssl(false)` fetcher option intentionally disables TLS
  verification and logs a warning; using it in production is not a
  vulnerability in the crate.
