use rust_scrapling::fetchers::response::Response;
use rust_scrapling::spiders::links::{canonicalize_url, LinkExtractor};
use rust_scrapling::spiders::response::SpiderResponse;
use std::collections::HashMap;
use std::sync::Arc;

fn page(url: &str, body: &str) -> SpiderResponse {
    SpiderResponse::new(Response::new(
        200,
        "text/html".to_string(),
        body.to_string(),
        url.to_string(),
        HashMap::new(),
    ))
}

#[test]
fn extracts_absolute_and_relative_links() {
    let resp = page(
        "https://example.com/dir/",
        r#"<html><body>
            <a href="https://example.com/a">A</a>
            <a href="/b">B</a>
            <a href="c">C</a>
        </body></html>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(
        urls,
        vec![
            "https://example.com/a",
            "https://example.com/b",
            "https://example.com/dir/c",
        ]
    );
}

#[test]
fn dedupes_preserving_document_order() {
    let resp = page(
        "https://example.com/",
        r#"<a href="/x">1</a><a href="/y">2</a><a href="/x">3</a>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/x", "https://example.com/y"]);
}

#[test]
fn allow_and_deny_patterns() {
    let resp = page(
        "https://example.com/",
        r#"<a href="/keep/1">k</a><a href="/keep/skip">s</a><a href="/drop/2">d</a>"#,
    );
    let urls = LinkExtractor::new()
        .allow([r"/keep/"])
        .unwrap()
        .deny([r"skip"])
        .unwrap()
        .extract(&resp);
    assert_eq!(urls, vec!["https://example.com/keep/1"]);
}

#[test]
fn domain_filters_match_subdomains() {
    let resp = page(
        "https://example.com/",
        r#"<a href="https://api.example.com/x">1</a>
           <a href="https://example.com/y">2</a>
           <a href="https://bad.example.org/z">3</a>
           <a href="https://internal.example.com/w">4</a>"#,
    );
    let urls = LinkExtractor::new()
        .allow_domains(["example.com"])
        .deny_domains(["internal.example.com"])
        .extract(&resp);
    assert_eq!(
        urls,
        vec!["https://api.example.com/x", "https://example.com/y"]
    );
}

#[test]
fn deny_extensions_defaults_drop_binaries() {
    let resp = page(
        "https://example.com/",
        r#"<a href="/report.pdf">pdf</a><a href="/photo.jpg">img</a><a href="/page.html">ok</a>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/page.html"]);
}

#[test]
fn compound_deny_extensions_are_honored() {
    // Regression for upstream fix 0d35a3f: `archive.tar.gz` must match a
    // deny list containing the compound extension `tar.gz`.
    let resp = page(
        "https://example.com/",
        r#"<a href="/archive.tar.gz">tar</a><a href="/data.gz.html">ok</a>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/data.gz.html"]);
}

#[test]
fn restrict_css_scopes_extraction() {
    let resp = page(
        "https://example.com/",
        r#"<div id="nav"><a href="/nav">n</a></div><div id="main"><a href="/main">m</a></div>"#,
    );
    let urls = LinkExtractor::new().restrict_css(["#main"]).extract(&resp);
    assert_eq!(urls, vec!["https://example.com/main"]);
}

#[test]
fn custom_tags_and_attrs() {
    let resp = page(
        "https://example.com/",
        r#"<img src="/pic.html"><a href="/normal">a</a>"#,
    );
    let urls = LinkExtractor::new()
        .tags(["img"])
        .attrs(["src"])
        .extract(&resp);
    assert_eq!(urls, vec!["https://example.com/pic.html"]);
}

#[test]
fn canonicalize_sorts_query_and_drops_fragment() {
    assert_eq!(
        canonicalize_url("https://example.com/p?b=2&a=1#frag", false),
        "https://example.com/p?a=1&b=2"
    );
    assert_eq!(
        canonicalize_url("https://example.com/p?b=2&a=1#frag", true),
        "https://example.com/p?a=1&b=2#frag"
    );
}

#[test]
fn extraction_canonicalizes_by_default() {
    let resp = page(
        "https://example.com/",
        r#"<a href="/p?b=2&a=1#s">x</a><a href="/p?a=1&b=2">y</a>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    // Both variants canonicalize to the same URL and dedupe to one.
    assert_eq!(urls, vec!["https://example.com/p?a=1&b=2"]);
}

#[test]
fn strips_whitespace_from_urls() {
    let resp = page(
        "https://example.com/",
        "<a href=\"  /padded  \">x</a><a href=\"   \">empty</a>",
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/padded"]);
}

#[test]
fn non_http_schemes_are_dropped() {
    let resp = page(
        "https://example.com/",
        r#"<a href="mailto:x@example.com">m</a><a href="javascript:void(0)">j</a><a href="/ok">k</a>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/ok"]);
}

#[test]
fn process_hook_can_rewrite_and_drop() {
    let resp = page(
        "https://example.com/",
        r#"<a href="/keep">k</a><a href="/drop">d</a>"#,
    );
    let urls = LinkExtractor::new()
        .process(Arc::new(|url: String| {
            if url.ends_with("/drop") {
                None
            } else {
                Some(url.replace("/keep", "/kept"))
            }
        }))
        .extract(&resp);
    assert_eq!(urls, vec!["https://example.com/kept"]);
}

#[test]
fn matches_filters_a_single_url() {
    let ex = LinkExtractor::new()
        .allow([r"/articles/"])
        .unwrap()
        .allow_domains(["example.com"]);
    assert!(ex.matches("https://example.com/articles/1"));
    assert!(!ex.matches("https://example.com/about"));
    assert!(!ex.matches("https://other.org/articles/1"));
    assert!(!ex.matches("https://example.com/articles/dump.tar.gz"));
}

#[test]
fn area_tags_are_extracted_by_default() {
    let resp = page(
        "https://example.com/",
        r#"<map><area href="/mapped"></map>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(urls, vec!["https://example.com/mapped"]);
}

#[test]
fn document_order_is_preserved_across_tags() {
    let resp = page(
        "https://example.com/",
        r#"<map><area href="/first"></map><a href="/second">a</a><map><area href="/third"></map>"#,
    );
    let urls = LinkExtractor::new().extract(&resp);
    assert_eq!(
        urls,
        vec![
            "https://example.com/first",
            "https://example.com/second",
            "https://example.com/third",
        ]
    );
}

#[test]
fn canonicalize_drops_empty_query_and_preserves_raw_bytes() {
    // `/p?` must dedupe with `/p`.
    assert_eq!(
        canonicalize_url("https://example.com/p?", false),
        "https://example.com/p"
    );
    assert_eq!(
        canonicalize_url("https://example.com/p?&&", false),
        "https://example.com/p"
    );
    // Percent-escapes that are not valid UTF-8 survive canonicalization.
    assert_eq!(
        canonicalize_url("https://example.com/?a=%FF", false),
        "https://example.com/?a=%FF"
    );
}
