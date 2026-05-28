use rust_scrapling::spiders::request::SpiderRequest;

#[test]
fn test_basic_creation() {
    let req = SpiderRequest::new("https://example.com/page");
    assert_eq!(req.url(), "https://example.com/page");
    assert_eq!(req.method(), "GET");
    assert_eq!(req.priority(), 0);
    assert!(!req.dont_filter());
    assert_eq!(req.retry_count(), 0);
    assert!(req.callback_name().is_none());
    assert!(req.body().is_none());
    assert!(!req.fingerprint().is_empty());
}

#[test]
fn test_builder() {
    let req = SpiderRequest::builder("https://example.com")
        .method("POST")
        .priority(5)
        .session_id("sess1")
        .callback("parse_item")
        .dont_filter(true)
        .header("Accept", "text/html")
        .body("{\"key\":\"value\"}")
        .meta("depth", serde_json::json!(2))
        .build();

    assert_eq!(req.method(), "POST");
    assert_eq!(req.priority(), 5);
    assert_eq!(req.session_id(), "sess1");
    assert_eq!(req.callback_name(), Some("parse_item"));
    assert!(req.dont_filter());
    assert_eq!(req.headers().get("Accept").unwrap(), "text/html");
    assert_eq!(req.body(), Some("{\"key\":\"value\"}"));
    assert_eq!(req.meta("depth"), Some(&serde_json::json!(2)));
}

#[test]
fn test_fingerprint_equality() {
    let r1 = SpiderRequest::new("https://example.com/page");
    let r2 = SpiderRequest::new("https://example.com/page");
    assert_eq!(r1, r2);
    assert_eq!(r1.fingerprint(), r2.fingerprint());
}

#[test]
fn test_fingerprint_inequality() {
    let r1 = SpiderRequest::new("https://example.com/page1");
    let r2 = SpiderRequest::new("https://example.com/page2");
    assert_ne!(r1, r2);
    assert_ne!(r1.fingerprint(), r2.fingerprint());
}

#[test]
fn test_copy() {
    let req = SpiderRequest::new("https://example.com");
    let copy = req.copy();
    assert_eq!(req, copy);
    assert_eq!(req.url(), copy.url());
}

#[test]
fn test_domain() {
    let req = SpiderRequest::new("https://www.example.com/path?q=1");
    assert_eq!(req.domain(), Some("www.example.com".to_string()));
}

#[test]
fn test_ordering() {
    let low = SpiderRequest::builder("https://example.com/low")
        .priority(1)
        .build();
    let high = SpiderRequest::builder("https://example.com/high")
        .priority(10)
        .build();
    assert!(high > low);
}

#[test]
fn test_meta() {
    let mut req = SpiderRequest::new("https://example.com");
    assert!(req.meta("key").is_none());
    req.set_meta("key", serde_json::json!("value"));
    assert_eq!(req.meta("key"), Some(&serde_json::json!("value")));
}

#[test]
fn test_setters() {
    let mut req = SpiderRequest::new("https://example.com");
    req.set_dont_filter(true);
    assert!(req.dont_filter());
    req.set_session_id("abc");
    assert_eq!(req.session_id(), "abc");
    req.increment_retry();
    assert_eq!(req.retry_count(), 1);
    req.increment_retry();
    assert_eq!(req.retry_count(), 2);
}

#[test]
fn test_priority_values() {
    let r1 = SpiderRequest::builder("https://a.com").priority(-5).build();
    let r2 = SpiderRequest::builder("https://b.com").priority(0).build();
    let r3 = SpiderRequest::builder("https://c.com").priority(5).build();
    assert!(r3 > r2);
    assert!(r2 > r1);
}
