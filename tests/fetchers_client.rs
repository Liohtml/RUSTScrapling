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
fn test_response_5xx_is_not_blocked() {
    // 5xx are genuine server errors, not bot blocks (#44). They must not be
    // classified as blocked, so the engine surfaces them via on_error rather
    // than silently burning the blocked-retry budget.
    for code in [500u16, 502, 503, 504] {
        let resp = Response::new(
            code,
            "text/html".to_string(),
            "Server Error".to_string(),
            "https://example.com".to_string(),
            HashMap::new(),
        );
        assert!(!resp.is_blocked(), "{} must not be is_blocked", code);
        assert!(!resp.is_success(), "{} is not success", code);
    }
}

#[test]
fn test_bot_block_codes_are_blocked() {
    for code in [401u16, 403, 407, 429, 444] {
        let resp = Response::new(
            code,
            "text/html".to_string(),
            "blocked".to_string(),
            "https://example.com".to_string(),
            HashMap::new(),
        );
        assert!(resp.is_blocked(), "{} should be is_blocked", code);
    }
}

#[test]
fn test_fetcher_builds_with_default_config() {
    // Fetcher::new now returns Result instead of panicking on build failure.
    assert!(Fetcher::new(FetcherConfig::default()).is_ok());
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
    assert!(Fetcher::new(config).is_ok());
}

#[tokio::test]
async fn session_manager_forwards_request_level_headers() {
    // Regression test: SpiderRequestBuilder::header() must actually reach the
    // wire. SessionManager::fetch used to call Fetcher::get/post/put/delete
    // without the request's headers, silently dropping them. A local TCP
    // listener stands in for a real server so this runs in CI without
    // network access.
    use rust_scrapling::spiders::request::SpiderRequest;
    use rust_scrapling::spiders::session::SessionManager;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            received.extend_from_slice(&buf[..n]);
            if received.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let body = "ok";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
        String::from_utf8_lossy(&received).to_string()
    });

    let mut manager = SessionManager::new(FetcherConfig::default());
    manager.ensure_default().unwrap();

    let request = SpiderRequest::builder(&format!("http://{}/", addr))
        .header("x-scrapling-test", "flows-through")
        .build();

    let response = manager.fetch(&request).await.expect("request succeeds");
    assert_eq!(response.status(), 200);

    let request_text = server.await.unwrap();
    assert!(
        request_text
            .to_lowercase()
            .contains("x-scrapling-test: flows-through"),
        "custom per-request header must reach the wire, got request:\n{request_text}"
    );
}

// Network tests (these hit httpbin.org - marked with #[ignore] so they don't run by default)
#[tokio::test]
#[ignore]
async fn test_fetcher_get() {
    let fetcher = Fetcher::new(FetcherConfig::default()).unwrap();
    let response = fetcher.get("https://httpbin.org/get").await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap().status(), 200);
}

#[tokio::test]
#[ignore]
async fn test_fetcher_post_json() {
    let fetcher = Fetcher::new(FetcherConfig::default()).unwrap();
    let body = serde_json::json!({"key": "value"});
    let response = fetcher
        .post("https://httpbin.org/post", None, Some(&body))
        .await;
    assert!(response.is_ok());
    let resp = response.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[ignore]
async fn test_fetcher_delete() {
    let fetcher = Fetcher::new(FetcherConfig::default()).unwrap();
    let response = fetcher.delete("https://httpbin.org/delete").await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap().status(), 200);
}
