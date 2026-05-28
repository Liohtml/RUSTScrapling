use rust_scrapling::core::storage::SqliteStorage;
use std::collections::HashMap;
use tempfile::tempdir;

fn make_data(key: &str, val: &str) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();
    map.insert(key.to_string(), serde_json::Value::String(val.to_string()));
    map
}

#[test]
fn test_save_and_retrieve_roundtrip() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let data = make_data("tag", "div");
    storage.save("elem1", &data).unwrap();

    let result = storage.retrieve("elem1").unwrap();
    assert!(result.is_some());
    let retrieved = result.unwrap();
    assert_eq!(
        retrieved.get("tag"),
        Some(&serde_json::Value::String("div".to_string()))
    );
}

#[test]
fn test_retrieve_nonexistent_returns_none() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let result = storage.retrieve("does_not_exist").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_update_existing_second_value_wins() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let first = make_data("class", "old-class");
    storage.save("elem1", &first).unwrap();

    let second = make_data("class", "new-class");
    storage.save("elem1", &second).unwrap();

    let result = storage.retrieve("elem1").unwrap().unwrap();
    assert_eq!(
        result.get("class"),
        Some(&serde_json::Value::String("new-class".to_string()))
    );
}

#[test]
fn test_different_urls_isolate_data() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let storage_a = SqliteStorage::new(db_path.to_str().unwrap(), "https://site-a.com").unwrap();
    let storage_b = SqliteStorage::new(db_path.to_str().unwrap(), "https://site-b.com").unwrap();

    let data_a = make_data("src", "site-a-value");
    storage_a.save("elem1", &data_a).unwrap();

    // site-b should not see site-a's data
    let result = storage_b.retrieve("elem1").unwrap();
    assert!(result.is_none());

    // site-a should still see its own data
    let result_a = storage_a.retrieve("elem1").unwrap();
    assert!(result_a.is_some());
    assert_eq!(
        result_a.unwrap().get("src"),
        Some(&serde_json::Value::String("site-a-value".to_string()))
    );
}

#[test]
fn test_different_identifiers_dont_collide() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = SqliteStorage::new(db_path.to_str().unwrap(), "https://example.com").unwrap();

    let data1 = make_data("id", "first");
    let data2 = make_data("id", "second");
    storage.save("identifier_one", &data1).unwrap();
    storage.save("identifier_two", &data2).unwrap();

    let result1 = storage.retrieve("identifier_one").unwrap().unwrap();
    let result2 = storage.retrieve("identifier_two").unwrap().unwrap();

    assert_eq!(
        result1.get("id"),
        Some(&serde_json::Value::String("first".to_string()))
    );
    assert_eq!(
        result2.get("id"),
        Some(&serde_json::Value::String("second".to_string()))
    );
}
