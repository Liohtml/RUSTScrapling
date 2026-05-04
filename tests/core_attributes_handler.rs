use rust_scrapling::core::AttributesHandler;

fn make_attrs() -> AttributesHandler {
    AttributesHandler::new(vec![
        ("class".to_string(), "btn primary".to_string()),
        ("id".to_string(), "submit-btn".to_string()),
        ("href".to_string(), "https://example.com".to_string()),
        ("data-value".to_string(), "42".to_string()),
    ])
}

#[test]
fn test_get_existing_key() {
    let attrs = make_attrs();
    let val = attrs.get("class").expect("class key should exist");
    assert_eq!(val.as_str(), "btn primary");
}

#[test]
fn test_get_missing_key_returns_none() {
    let attrs = make_attrs();
    assert!(attrs.get("nonexistent").is_none());
}

#[test]
fn test_contains_key_true() {
    let attrs = make_attrs();
    assert!(attrs.contains_key("id"));
}

#[test]
fn test_contains_key_false() {
    let attrs = make_attrs();
    assert!(!attrs.contains_key("style"));
}

#[test]
fn test_len() {
    let attrs = make_attrs();
    assert_eq!(attrs.len(), 4);
}

#[test]
fn test_is_empty_false() {
    let attrs = make_attrs();
    assert!(!attrs.is_empty());
}

#[test]
fn test_is_empty_true() {
    let attrs = AttributesHandler::new(vec![]);
    assert!(attrs.is_empty());
    assert_eq!(attrs.len(), 0);
}

#[test]
fn test_keys_iteration() {
    let attrs = make_attrs();
    let keys: Vec<&str> = attrs.keys().collect();
    assert_eq!(keys, vec!["class", "id", "href", "data-value"]);
}

#[test]
fn test_values_iteration() {
    let attrs = make_attrs();
    let values: Vec<&str> = attrs.values().map(|v| v.as_str()).collect();
    assert_eq!(values, vec!["btn primary", "submit-btn", "https://example.com", "42"]);
}

#[test]
fn test_iter() {
    let attrs = make_attrs();
    let pairs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (k, v.as_str())).collect();
    assert_eq!(pairs[0], ("class", "btn primary"));
    assert_eq!(pairs[1], ("id", "submit-btn"));
}

#[test]
fn test_search_values_exact_match() {
    let attrs = make_attrs();
    let results: Vec<(&str, &str)> = attrs
        .search_values("submit-btn", false)
        .map(|(k, v)| (k, v.as_str()))
        .collect();
    assert_eq!(results, vec![("id", "submit-btn")]);
}

#[test]
fn test_search_values_exact_no_match() {
    let attrs = make_attrs();
    let results: Vec<_> = attrs.search_values("btn", false).collect();
    assert!(results.is_empty());
}

#[test]
fn test_search_values_partial_match() {
    let attrs = make_attrs();
    let results: Vec<(&str, &str)> = attrs
        .search_values("btn", true)
        .map(|(k, v)| (k, v.as_str()))
        .collect();
    // matches "btn primary" (class) and "submit-btn" (id)
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|(k, _)| *k == "class"));
    assert!(results.iter().any(|(k, _)| *k == "id"));
}

#[test]
fn test_search_values_partial_no_match() {
    let attrs = make_attrs();
    let results: Vec<_> = attrs.search_values("zzz", true).collect();
    assert!(results.is_empty());
}

#[test]
fn test_json_string() {
    let attrs = AttributesHandler::new(vec![
        ("id".to_string(), "foo".to_string()),
        ("class".to_string(), "bar".to_string()),
    ]);
    let json = attrs.json_string();
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed["id"], "foo");
    assert_eq!(parsed["class"], "bar");
}

#[test]
fn test_index_trait() {
    let attrs = make_attrs();
    let val = &attrs["href"];
    assert_eq!(val.as_str(), "https://example.com");
}

#[test]
fn test_new_from_hashmap() {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert("lang".to_string(), "en".to_string());
    map.insert("dir".to_string(), "ltr".to_string());
    let attrs = AttributesHandler::new(map);
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs.get("lang").unwrap().as_str(), "en");
    assert_eq!(attrs.get("dir").unwrap().as_str(), "ltr");
}
