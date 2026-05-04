# Contributing to RUSTScrapling

Thank you for your interest in contributing to RUSTScrapling! This document provides guidelines and information for contributors.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/RUSTScrapling.git
   cd RUSTScrapling
   ```
3. Build and run tests:
   ```bash
   cargo build
   cargo test
   ```

## Development Workflow

1. Create a feature branch from `master`:
   ```bash
   git checkout -b feature/your-feature-name
   ```
2. Make your changes, following the code style below
3. Write tests for new functionality
4. Run the full test suite:
   ```bash
   cargo test
   cargo clippy -- -W clippy::all
   ```
5. Commit with a descriptive message:
   ```bash
   git commit -m "feat: add support for XPath selectors"
   ```
6. Push and open a Pull Request

## Code Style

- Follow standard Rust conventions (`rustfmt` defaults)
- Use `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Public APIs must have doc comments
- Keep functions focused and small
- Prefer returning `Result` over panicking

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` -- new feature
- `fix:` -- bug fix
- `docs:` -- documentation only
- `test:` -- adding or updating tests
- `refactor:` -- code change that neither fixes a bug nor adds a feature
- `perf:` -- performance improvement
- `chore:` -- maintenance tasks

## Testing

- Every new public method needs at least one test
- Place integration tests in `tests/`
- Place unit tests inline with `#[cfg(test)]` modules
- Network-dependent tests must be marked `#[ignore]`
- Use `tempfile` for tests that need filesystem access

## Pull Request Guidelines

- Keep PRs focused on a single change
- Include a description of what changed and why
- Reference any related issues
- Ensure CI passes (tests + clippy)
- Be responsive to review feedback

## Reporting Issues

When filing an issue, include:

- Rust version (`rustc --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Relevant error messages or logs

## Areas Looking for Help

- **Browser automation** -- Headless Chrome / Playwright integration
- **Adaptive element relocation** -- Similarity scoring for finding moved elements
- **Performance benchmarks** -- Comparing against Python Scrapling and other Rust parsers
- **XPath support** -- Full XPath query engine
- **Documentation** -- More examples and tutorials

## License

By contributing, you agree that your contributions will be licensed under the same dual MIT/Apache-2.0 license as the project.
