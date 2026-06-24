use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::fetchers::proxy::ProxyRotator;

// ---------------------------------------------------------------------------
// FetcherConfig – defaults
// ---------------------------------------------------------------------------

#[test]
fn default_config_values() {
    let cfg = FetcherConfig::default();
    assert_eq!(cfg.timeout_secs, 30);
    assert_eq!(cfg.retries, 3);
    assert_eq!(cfg.retry_delay_secs, 1);
    assert!(cfg.follow_redirects);
    assert_eq!(cfg.max_redirects, 10);
    assert!(cfg.verify_ssl);
    assert!(cfg.proxy.is_none());
    assert!(cfg.proxies.is_empty());
    assert!(cfg.headers.is_empty());
    assert!(cfg.stealthy_headers);
    assert!(cfg.user_agent.is_none());
}

// ---------------------------------------------------------------------------
// FetcherConfig – builder pattern
// ---------------------------------------------------------------------------

#[test]
fn builder_overrides_defaults() {
    let cfg = FetcherConfig::builder()
        .timeout(60)
        .retries(5)
        .retry_delay(2)
        .proxy("http://proxy.example.com:8080")
        .user_agent("TestBot/1.0")
        .stealth(false)
        .follow_redirects(false)
        .verify_ssl(false)
        .header("x-custom", "value")
        .build();

    assert_eq!(cfg.timeout_secs, 60);
    assert_eq!(cfg.retries, 5);
    assert_eq!(cfg.retry_delay_secs, 2);
    assert_eq!(cfg.proxy.as_deref(), Some("http://proxy.example.com:8080"));
    assert_eq!(cfg.user_agent.as_deref(), Some("TestBot/1.0"));
    assert!(!cfg.stealthy_headers);
    assert!(!cfg.follow_redirects);
    assert!(!cfg.verify_ssl);
    assert_eq!(
        cfg.headers.get("x-custom").map(|s| s.as_str()),
        Some("value")
    );
}

#[test]
fn builder_default_produces_same_as_default() {
    let via_builder = FetcherConfig::builder().build();
    let direct = FetcherConfig::default();
    assert_eq!(via_builder.timeout_secs, direct.timeout_secs);
    assert_eq!(via_builder.retries, direct.retries);
    assert_eq!(via_builder.stealthy_headers, direct.stealthy_headers);
}

// ---------------------------------------------------------------------------
// build_headers – stealth mode
// ---------------------------------------------------------------------------

#[test]
fn stealth_headers_contain_required_fields() {
    let cfg = FetcherConfig::default();
    let headers = cfg.build_headers("https://example.com", true);

    assert!(
        headers.contains_key("user-agent"),
        "user-agent must be present"
    );
    assert!(headers.contains_key("accept"), "accept must be present");
    assert!(
        headers.contains_key("accept-language"),
        "accept-language must be present"
    );
    assert!(
        headers.contains_key("accept-encoding"),
        "accept-encoding must be present"
    );
    assert!(
        headers.contains_key("sec-fetch-dest"),
        "sec-fetch-dest must be present"
    );
    assert!(
        headers.contains_key("sec-fetch-mode"),
        "sec-fetch-mode must be present"
    );
    assert!(
        headers.contains_key("sec-fetch-site"),
        "sec-fetch-site must be present"
    );
}

#[test]
fn stealth_user_agent_is_non_empty() {
    let cfg = FetcherConfig::default();
    let headers = cfg.build_headers("https://example.com", true);
    let ua = headers
        .get("user-agent")
        .expect("user-agent header missing");
    assert!(!ua.is_empty());
}

// ---------------------------------------------------------------------------
// build_headers – non-stealth mode
// ---------------------------------------------------------------------------

#[test]
fn non_stealth_headers_still_have_user_agent() {
    let cfg = FetcherConfig::builder().stealth(false).build();
    let headers = cfg.build_headers("https://example.com", false);

    assert!(
        headers.contains_key("user-agent"),
        "user-agent must be present even without stealth"
    );
    assert!(
        !headers.contains_key("sec-fetch-dest"),
        "sec-fetch-dest should be absent without stealth"
    );
}

#[test]
fn custom_user_agent_is_used_when_set() {
    let cfg = FetcherConfig::builder()
        .user_agent("MyCustomAgent/2.0")
        .build();
    let headers = cfg.build_headers("https://example.com", false);
    assert_eq!(
        headers.get("user-agent").map(|s| s.as_str()),
        Some("MyCustomAgent/2.0")
    );
}

#[test]
fn custom_header_is_preserved_in_stealth_mode() {
    let cfg = FetcherConfig::builder()
        .header("x-request-id", "abc123")
        .build();
    let headers = cfg.build_headers("https://example.com", true);
    assert_eq!(
        headers.get("x-request-id").map(|s| s.as_str()),
        Some("abc123")
    );
}

// ---------------------------------------------------------------------------
// ProxyRotator
// ---------------------------------------------------------------------------

#[test]
fn proxy_rotator_returns_none_for_empty_list() {
    assert!(ProxyRotator::new(vec![]).is_none());
}

#[test]
fn proxy_rotator_round_robin() {
    let proxies = vec![
        "http://proxy1.example.com".to_string(),
        "http://proxy2.example.com".to_string(),
        "http://proxy3.example.com".to_string(),
    ];
    let rotator = ProxyRotator::new(proxies.clone()).expect("should create rotator");

    // First pass – expect sequential order.
    for proxy in &proxies {
        assert_eq!(rotator.next(), proxy.as_str());
    }
    // Second pass – wraps around.
    assert_eq!(rotator.next(), proxies[0].as_str());
}

#[test]
fn proxy_rotator_random_stays_in_bounds() {
    let proxies: Vec<String> = (0..5)
        .map(|i| format!("http://proxy{}.example.com", i))
        .collect();
    let rotator = ProxyRotator::new(proxies).expect("should create rotator");

    for _ in 0..20 {
        let p = rotator.random();
        assert!(p.starts_with("http://proxy"), "unexpected proxy: {}", p);
    }
}

#[test]
fn proxy_rotator_len() {
    let proxies = vec![
        "http://a.example.com".to_string(),
        "http://b.example.com".to_string(),
    ];
    let rotator = ProxyRotator::new(proxies).unwrap();
    assert_eq!(rotator.len(), 2);
    assert!(!rotator.is_empty());
}

#[test]
fn proxy_rotator_next_index_round_robin() {
    let proxies = vec![
        "http://a.example.com".to_string(),
        "http://b.example.com".to_string(),
        "http://c.example.com".to_string(),
    ];
    let rotator = ProxyRotator::new(proxies).unwrap();
    assert_eq!(rotator.next_index(), 0);
    assert_eq!(rotator.next_index(), 1);
    assert_eq!(rotator.next_index(), 2);
    assert_eq!(rotator.next_index(), 0);
}

// ---------------------------------------------------------------------------
// FetcherConfig – proxy configuration
// ---------------------------------------------------------------------------

#[test]
fn builder_protocol_proxy_populates_map() {
    let cfg = FetcherConfig::builder()
        .protocol_proxy("HTTP", "http://p1.example.com")
        .protocol_proxy("https", "http://p2.example.com")
        .build();
    assert_eq!(
        cfg.proxies.get("http").map(|s| s.as_str()),
        Some("http://p1.example.com")
    );
    assert_eq!(
        cfg.proxies.get("https").map(|s| s.as_str()),
        Some("http://p2.example.com")
    );
}

#[test]
fn builder_rotating_proxies_populates_list() {
    let cfg = FetcherConfig::builder()
        .rotating_proxies(vec!["http://p1.example.com", "http://p2.example.com"])
        .build();
    assert_eq!(
        cfg.proxy_list,
        vec!["http://p1.example.com", "http://p2.example.com"]
    );
}

#[test]
fn default_proxy_list_is_empty() {
    assert!(FetcherConfig::default().proxy_list.is_empty());
}

#[test]
fn default_max_body_bytes_is_50_mib() {
    assert_eq!(FetcherConfig::default().max_body_bytes, 50 * 1024 * 1024);
}

#[test]
fn builder_max_body_bytes_overrides_default() {
    let cfg = FetcherConfig::builder().max_body_bytes(1024).build();
    assert_eq!(cfg.max_body_bytes, 1024);
}

// ---------------------------------------------------------------------------
// build_headers – Client-Hints consistency (#36)
// ---------------------------------------------------------------------------

#[test]
fn chromium_ua_gets_client_hints_with_derived_platform() {
    let cfg = FetcherConfig::builder()
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .build();
    let headers = cfg.build_headers("https://example.com", true);
    assert!(
        headers.contains_key("sec-ch-ua"),
        "Chrome UA should get sec-ch-ua"
    );
    assert_eq!(
        headers.get("sec-ch-ua-platform").map(|s| s.as_str()),
        Some("\"macOS\""),
        "platform should be derived from the Mac UA, not hardcoded Windows"
    );
}

#[test]
fn firefox_ua_does_not_get_client_hints() {
    let cfg = FetcherConfig::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
        )
        .build();
    let headers = cfg.build_headers("https://example.com", true);
    assert!(
        !headers.contains_key("sec-ch-ua"),
        "Firefox does not implement Client Hints; sending sec-ch-ua reveals a bot"
    );
    assert!(
        !headers.contains_key("sec-ch-ua-platform"),
        "Firefox must not send sec-ch-ua-platform"
    );
    // Fetch-Metadata headers are still sent (Firefox supports them).
    assert!(headers.contains_key("sec-fetch-mode"));
}
