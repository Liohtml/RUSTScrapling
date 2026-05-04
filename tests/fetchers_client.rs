use rust_scrapling::fetchers::client::Fetcher;
use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::fetchers::response::Response;
use std::collections::HashMap;

// Test Response struct directly (no network needed)
#[test]
fn test_response_struct() {
    let resp = Response::new(
        200,
        "text/html".to_string(),
        "<html><body><h1>Test</h1></body></html>".to_string(),
        "https://example.com".to_string(),
        HashMap::new(),
    );
    assert_eq!(resp.status(), 200);
    assert!(resp.is_success());
    assert!(!resp.is_blocked());
    let sel = resp.selector();
    let h1 = sel.css("h1");
    assert_eq!(h1[0].text().as_str(), "Test");
}

#[test]
fn test_response_json() {
    let resp = Response::new(
        200,
        "application/json".to_string(),
        r#"{"key":"value"}"#.to_string(),
        "https://api.example.com".to_string(),
        HashMap::new(),
    );
    let json = resp.json().unwrap();
    assert_eq!(json["key"], "value");
}

#[test]
fn test_response_blocked() {
    let resp = Response::new(
        403,
        "text/html".to_string(),
        "Forbidden".to_string(),
        "https://example.com".to_string(),
        HashMap::new(),
    );
    assert!(resp.is_blocked());
    assert!(!resp.is_success());
}

#[test]
fn test_response_content_length() {
    let body = "Hello, World!";
    let resp = Response::new(
        200,
        "text/plain".to_string(),
        body.to_string(),
        "https://example.com".to_string(),
        HashMap::new(),
    );
    assert_eq!(resp.content_length(), body.len());
}

#[test]
fn test_response_url_and_content_type() {
    let resp = Response::new(
        200,
        "application/json".to_string(),
        "{}".to_string(),
        "https://api.example.com/data".to_string(),
        HashMap::new(),
    );
    assert_eq!(resp.url(), "https://api.example.com/data");
    assert_eq!(resp.content_type(), "application/json");
}

#[test]
fn test_response_headers() {
    let mut headers = HashMap::new();
    headers.insert("x-custom-header".to_string(), "test-value".to_string());
    let resp = Response::new(
        200,
        "text/html".to_string(),
        "<html></html>".to_string(),
        "https://example.com".to_string(),
        headers,
    );
    assert_eq!(
        resp.headers().get("x-custom-header").map(|s| s.as_str()),
        Some("test-value")
    );
}

#[test]
fn test_response_500_is_blocked() {
    let resp = Response::new(
        500,
        "text/html".to_string(),
        "Internal Server Error".to_string(),
        "https://example.com".to_string(),
        HashMap::new(),
    );
    assert!(resp.is_blocked());
    assert!(!resp.is_success());
}

#[test]
fn test_fetcher_builds_with_default_config() {
    // Just verify that Fetcher::new doesn't panic with default config
    let _fetcher = Fetcher::new(FetcherConfig::default());
}

#[test]
fn test_fetcher_builds_with_custom_config() {
    let config = FetcherConfig::builder()
        .timeout(60)
        .retries(5)
        .retry_delay(2)
        .follow_redirects(false)
        .verify_ssl(false)
        .stealth(false)
        .build();
    let _fetcher = Fetcher::new(config);
}

// Network tests (these hit httpbin.org - marked with #[ignore] so they don't run by default)
#[tokio::test]
#[ignore]
async fn test_fetcher_get() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher.get("https://httpbin.org/get").await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap().status(), 200);
}

#[tokio::test]
#[ignore]
async fn test_fetcher_post_json() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let body = serde_json::json!({"key": "value"});
    let response = fetcher.post("https://httpbin.org/post", None, Some(&body)).await;
    assert!(response.is_ok());
    let resp = response.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[ignore]
async fn test_fetcher_delete() {
    let fetcher = Fetcher::new(FetcherConfig::default());
    let response = fetcher.delete("https://httpbin.org/delete").await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap().status(), 200);
}
