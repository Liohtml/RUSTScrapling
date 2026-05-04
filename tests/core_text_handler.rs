use rust_scrapling::core::{TextHandler, TextHandlers};

// ── TextHandler basic constructors and accessors ──

#[test]
fn test_new_and_as_str() {
    let t = TextHandler::new("hello");
    assert_eq!(t.as_str(), "hello");
}

#[test]
fn test_from_string() {
    let t: TextHandler = String::from("world").into();
    assert_eq!(t.as_str(), "world");
}

#[test]
fn test_from_str() {
    let t: TextHandler = "test".into();
    assert_eq!(t.as_str(), "test");
}

#[test]
fn test_into_string() {
    let t = TextHandler::new("hello");
    let s: String = t.into_string();
    assert_eq!(s, "hello");
}

#[test]
fn test_is_empty() {
    assert!(TextHandler::new("").is_empty());
    assert!(!TextHandler::new("x").is_empty());
}

#[test]
fn test_len() {
    assert_eq!(TextHandler::new("").len(), 0);
    assert_eq!(TextHandler::new("abc").len(), 3);
}

// ── String manipulation ──

#[test]
fn test_strip() {
    let t = TextHandler::new("  hello  ");
    assert_eq!(t.strip().as_str(), "hello");
}

#[test]
fn test_to_lowercase() {
    let t = TextHandler::new("HELLO World");
    assert_eq!(t.to_lowercase().as_str(), "hello world");
}

#[test]
fn test_to_uppercase() {
    let t = TextHandler::new("hello World");
    assert_eq!(t.to_uppercase().as_str(), "HELLO WORLD");
}

#[test]
fn test_contains_str() {
    let t = TextHandler::new("hello world");
    assert!(t.contains_str("world"));
    assert!(!t.contains_str("xyz"));
}

#[test]
fn test_replace_str() {
    let t = TextHandler::new("hello world");
    assert_eq!(t.replace_str("world", "rust").as_str(), "hello rust");
}

#[test]
fn test_starts_with_str() {
    let t = TextHandler::new("hello world");
    assert!(t.starts_with_str("hello"));
    assert!(!t.starts_with_str("world"));
}

#[test]
fn test_ends_with_str() {
    let t = TextHandler::new("hello world");
    assert!(t.ends_with_str("world"));
    assert!(!t.ends_with_str("hello"));
}

#[test]
fn test_split_str() {
    let t = TextHandler::new("a,b,c");
    let parts = t.split_str(",");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].as_str(), "a");
    assert_eq!(parts[1].as_str(), "b");
    assert_eq!(parts[2].as_str(), "c");
}

// ── clean ──

#[test]
fn test_clean_whitespace() {
    let t = TextHandler::new("  hello\t\nworld  \r\n  foo  ");
    let cleaned = t.clean(false);
    assert_eq!(cleaned.as_str(), "hello world foo");
}

#[test]
fn test_clean_with_entities() {
    let t = TextHandler::new("Hello &amp; World &lt;tag&gt;");
    let cleaned = t.clean(true);
    assert_eq!(cleaned.as_str(), "Hello & World <tag>");
}

#[test]
fn test_clean_without_entities() {
    let t = TextHandler::new("Hello &amp; World");
    let cleaned = t.clean(false);
    assert_eq!(cleaned.as_str(), "Hello &amp; World");
}

// ── JSON ──

#[test]
fn test_json_valid() {
    let t = TextHandler::new(r#"{"key": "value", "num": 42}"#);
    let val = t.json().unwrap();
    assert_eq!(val["key"], "value");
    assert_eq!(val["num"], 42);
}

#[test]
fn test_json_invalid() {
    let t = TextHandler::new("not json");
    assert!(t.json().is_err());
}

// ── Regex ──

#[test]
fn test_re_no_capture_group() {
    let t = TextHandler::new("price: 100 and 200 dollars");
    let results = t.re(r"\d+", false, false, true);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].as_str(), "100");
    assert_eq!(results[1].as_str(), "200");
}

#[test]
fn test_re_with_capture_group() {
    let t = TextHandler::new("price: 100 dollars and 200 euros");
    let results = t.re(r"(\d+) dollars", false, false, true);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_str(), "100");
}

#[test]
fn test_re_case_insensitive() {
    let t = TextHandler::new("Hello WORLD hello");
    let results = t.re("hello", false, false, false);
    assert_eq!(results.len(), 2);
}

#[test]
fn test_re_case_sensitive() {
    let t = TextHandler::new("Hello WORLD hello");
    let results = t.re("hello", false, false, true);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_str(), "hello");
}

#[test]
fn test_re_with_entity_replacement() {
    let t = TextHandler::new("5 &gt; 3 &amp; 2 &lt; 4");
    let results = t.re(r"\d+ > \d+", true, false, true);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_str(), "5 > 3");
}

#[test]
fn test_re_clean_match() {
    let t = TextHandler::new("value:  hello  ");
    let results = t.re(r"value:(.+)", false, true, true);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_str(), "hello");
}

#[test]
fn test_re_invalid_pattern() {
    let t = TextHandler::new("test");
    let results = t.re(r"[invalid", false, false, true);
    assert!(results.is_empty());
}

#[test]
fn test_re_first_match() {
    let t = TextHandler::new("abc 123 def 456");
    let first = t.re_first(r"\d+", false, false, true);
    assert_eq!(first.unwrap().as_str(), "123");
}

#[test]
fn test_re_first_no_match() {
    let t = TextHandler::new("no numbers here");
    let first = t.re_first(r"\d+", false, false, true);
    assert!(first.is_none());
}

// ── Display ──

#[test]
fn test_display() {
    let t = TextHandler::new("hello");
    assert_eq!(format!("{}", t), "hello");
}

// ── get / getall ──

#[test]
fn test_get_and_getall() {
    let t = TextHandler::new("test");
    assert_eq!(t.get().as_str(), "test");
    let all = t.getall();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].as_str(), "test");
}

// ── TextHandlers ──

#[test]
fn test_text_handlers_new_and_len() {
    let items = vec![TextHandler::new("a"), TextHandler::new("b")];
    let handlers = TextHandlers::new(items);
    assert_eq!(handlers.len(), 2);
    assert!(!handlers.is_empty());
}

#[test]
fn test_text_handlers_empty() {
    let handlers = TextHandlers::default();
    assert!(handlers.is_empty());
    assert_eq!(handlers.len(), 0);
}

#[test]
fn test_text_handlers_get_with_items() {
    let handlers = TextHandlers::new(vec![TextHandler::new("first"), TextHandler::new("second")]);
    let val = handlers.get(None);
    assert_eq!(val.unwrap().as_str(), "first");
}

#[test]
fn test_text_handlers_get_empty_with_default() {
    let handlers = TextHandlers::default();
    let val = handlers.get(Some(TextHandler::new("default")));
    assert_eq!(val.unwrap().as_str(), "default");
}

#[test]
fn test_text_handlers_get_empty_no_default() {
    let handlers = TextHandlers::default();
    assert!(handlers.get(None).is_none());
}

#[test]
fn test_text_handlers_getall() {
    let handlers = TextHandlers::new(vec![TextHandler::new("a"), TextHandler::new("b")]);
    let all = handlers.getall();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].as_str(), "a");
    assert_eq!(all[1].as_str(), "b");
}

#[test]
fn test_text_handlers_index() {
    let handlers = TextHandlers::new(vec![TextHandler::new("x"), TextHandler::new("y")]);
    assert_eq!(handlers[0].as_str(), "x");
    assert_eq!(handlers[1].as_str(), "y");
}

#[test]
fn test_text_handlers_re() {
    let handlers = TextHandlers::new(vec![
        TextHandler::new("price: 100 dollars"),
        TextHandler::new("cost: 200 euros"),
    ]);
    let results = handlers.re(r"\d+", false, false, true);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].as_str(), "100");
    assert_eq!(results[1].as_str(), "200");
}

#[test]
fn test_text_handlers_re_first() {
    let handlers = TextHandlers::new(vec![
        TextHandler::new("no match here"),
        TextHandler::new("found 42"),
    ]);
    let first = handlers.re_first(r"\d+", false, false, true);
    assert_eq!(first.unwrap().as_str(), "42");
}

#[test]
fn test_text_handlers_re_first_no_match() {
    let handlers = TextHandlers::new(vec![
        TextHandler::new("no numbers"),
        TextHandler::new("still none"),
    ]);
    assert!(handlers.re_first(r"\d+", false, false, true).is_none());
}

#[test]
fn test_text_handlers_into_iter() {
    let handlers = TextHandlers::new(vec![TextHandler::new("a"), TextHandler::new("b")]);
    let collected: Vec<String> = handlers.into_iter().map(|t| t.into_string()).collect();
    assert_eq!(collected, vec!["a", "b"]);
}

#[test]
fn test_text_handlers_ref_iter() {
    let handlers = TextHandlers::new(vec![TextHandler::new("a"), TextHandler::new("b")]);
    let collected: Vec<&str> = (&handlers).into_iter().map(|t| t.as_str()).collect();
    assert_eq!(collected, vec!["a", "b"]);
}
