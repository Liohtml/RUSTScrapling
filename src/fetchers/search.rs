//! Helpers for scraping search-engine result pages.
//!
//! Currently this covers DuckDuckGo's HTML endpoint
//! (`html.duckduckgo.com/html/`), which has two quirks that silently break
//! naive scrapers:
//!
//! 1. On bot detection it returns **HTTP 202 with its homepage** instead of an
//!    error or CAPTCHA, so a plain `status()`/`is_blocked()` check misses it
//!    (see [`is_duckduckgo_blocked`]).
//! 2. Every organic result link is wrapped in an internal redirect
//!    (`//duckduckgo.com/l/?uddg=<percent-encoded-target>`); the real URL must
//!    be decoded from the `uddg` query parameter (see [`decode_duckduckgo_href`]).
//!
//! For reliable automated search, prefer Startpage or the DuckDuckGo Instant
//! Answers API — DuckDuckGo's HTML endpoint rate-limits aggressively even with
//! stealth headers.

use crate::fetchers::response::Response;

/// Detect DuckDuckGo's soft-block, where it answers a search query with HTTP
/// 202 and its homepage HTML rather than results — a case the generic
/// [`Response::is_blocked`](crate::fetchers::response::Response::is_blocked)
/// cannot catch because 202 is nominally a success code.
///
/// Returns `false` for non-DuckDuckGo responses, so it is safe to call on any
/// response.
pub fn is_duckduckgo_blocked(response: &Response) -> bool {
    if !response.url().contains("duckduckgo.com") {
        return false;
    }
    // The documented soft-block signal: a 202 carrying the homepage.
    if response.status() == 202 {
        return true;
    }
    // Or a 2xx body that has no result links — i.e. the homepage was served
    // in place of results.
    response.is_success() && !response.text().contains("result__a")
}

/// Decode a DuckDuckGo result `href` to its real target URL.
///
/// DuckDuckGo wraps result links as `//duckduckgo.com/l/?uddg=<encoded>&rut=…`.
/// This prepends a scheme to protocol-relative hrefs, then extracts and
/// percent-decodes the `uddg` parameter. Any href that is not a DuckDuckGo
/// redirect is returned unchanged.
pub fn decode_duckduckgo_href(href: &str) -> String {
    let full = if let Some(stripped) = href.strip_prefix("//") {
        format!("https://{}", stripped)
    } else {
        href.to_string()
    };

    if let Ok(parsed) = url::Url::parse(&full) {
        let is_ddg_redirect = parsed
            .host_str()
            .map(|h| h.contains("duckduckgo.com"))
            .unwrap_or(false)
            && parsed.path() == "/l/";
        if is_ddg_redirect {
            if let Some((_, target)) = parsed.query_pairs().find(|(k, _)| k == "uddg") {
                return target.into_owned();
            }
        }
    }
    href.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resp(status: u16, url: &str, body: &str) -> Response {
        Response::new(
            status,
            "text/html".to_string(),
            body.to_string(),
            url.to_string(),
            HashMap::new(),
        )
    }

    #[test]
    fn decodes_ddg_redirect_href() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.andritz.com%2F&rut=abc";
        assert_eq!(decode_duckduckgo_href(href), "https://www.andritz.com/");
    }

    #[test]
    fn passes_through_non_ddg_href() {
        assert_eq!(
            decode_duckduckgo_href("https://example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn passes_through_ddg_non_redirect_path() {
        // Not the /l/ redirect endpoint — leave it alone.
        let href = "//duckduckgo.com/about";
        assert_eq!(decode_duckduckgo_href(href), href);
    }

    #[test]
    fn detects_202_soft_block() {
        let r = resp(
            202,
            "https://html.duckduckgo.com/html/?q=x",
            "<html>home</html>",
        );
        assert!(is_duckduckgo_blocked(&r));
    }

    #[test]
    fn detects_homepage_without_results() {
        let r = resp(
            200,
            "https://html.duckduckgo.com/html/?q=x",
            "<html>no results here</html>",
        );
        assert!(is_duckduckgo_blocked(&r));
    }

    #[test]
    fn real_ddg_results_are_not_blocked() {
        let r = resp(
            200,
            "https://html.duckduckgo.com/html/?q=x",
            "<a class=\"result__a\" href=\"//duckduckgo.com/l/?uddg=https%3A%2F%2Fx\">X</a>",
        );
        assert!(!is_duckduckgo_blocked(&r));
    }

    #[test]
    fn non_ddg_response_is_never_ddg_blocked() {
        let r = resp(202, "https://example.com", "");
        assert!(!is_duckduckgo_blocked(&r));
    }
}
