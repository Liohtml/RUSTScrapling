#!/usr/bin/env bash
# Local CI: mirrors .github/workflows/ci.yml so the same checks can run
# without GitHub Actions (locally, in a git hook, or any environment).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

echo "==> cargo fmt -- --check"
cargo fmt -- --check

echo "==> cargo clippy --all-targets -- -W clippy::all -D warnings"
cargo clippy --all-targets -- -W clippy::all -D warnings

echo "==> cargo build --verbose"
cargo build --verbose

echo "==> cargo test --verbose"
cargo test --verbose

echo "All checks passed."
