use rust_scrapling::spiders::result::{CrawlResult, CrawlStats, ItemList};
use std::time::Instant;

#[test]
fn test_item_list_push_len() {
    let mut list = ItemList::new();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);

    list.push(serde_json::json!({"name": "item1"}));
    list.push(serde_json::json!({"name": "item2"}));
    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
}

#[test]
fn test_item_list_into_iter() {
    let mut list = ItemList::new();
    list.push(serde_json::json!(1));
    list.push(serde_json::json!(2));

    let collected: Vec<_> = list.into_iter().collect();
    assert_eq!(collected.len(), 2);
}

#[test]
fn test_item_list_to_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("items.json");

    let mut list = ItemList::new();
    list.push(serde_json::json!({"a": 1}));
    list.push(serde_json::json!({"b": 2}));

    list.to_json(&path, 2).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.len(), 2);
}

#[test]
fn test_item_list_to_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("items.jsonl");

    let mut list = ItemList::new();
    list.push(serde_json::json!({"x": 1}));
    list.push(serde_json::json!({"y": 2}));

    list.to_jsonl(&path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["x"], 1);
}

#[test]
fn test_crawl_stats_default() {
    let stats = CrawlStats::default();
    assert_eq!(stats.requests_count, 0);
    assert_eq!(stats.failed_requests_count, 0);
    assert_eq!(stats.response_bytes, 0);
    assert!(stats.start_time.is_none());
    assert!(stats.end_time.is_none());
}

#[test]
fn test_crawl_stats_increment() {
    let mut stats = CrawlStats::default();
    stats.increment_requests_count();
    stats.increment_requests_count();
    assert_eq!(stats.requests_count, 2);

    stats.increment_status(200);
    stats.increment_status(200);
    stats.increment_status(404);
    assert_eq!(stats.response_status_count[&200], 2);
    assert_eq!(stats.response_status_count[&404], 1);

    stats.increment_response_bytes(1024);
    stats.increment_response_bytes(512);
    assert_eq!(stats.response_bytes, 1536);
}

#[test]
fn test_crawl_stats_elapsed() {
    let mut stats = CrawlStats::default();
    assert_eq!(stats.elapsed_seconds(), 0.0);

    stats.start_time = Some(Instant::now());
    // elapsed should be > 0 (measuring from start to now)
    assert!(stats.elapsed_seconds() >= 0.0);
}

#[test]
fn test_crawl_result_completed() {
    let result = CrawlResult {
        stats: CrawlStats::default(),
        items: ItemList::new(),
        paused: false,
    };
    assert!(result.completed());

    let paused = CrawlResult {
        stats: CrawlStats::default(),
        items: ItemList::new(),
        paused: true,
    };
    assert!(!paused.completed());
}
