//! End-to-end tests for `ShopifySpider`, run through the real crawl engine
//! in development mode with a pre-seeded response cache (no network I/O).
//! Fixtures mirror upstream Scrapling's `test_shopify.py`.

use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::spiders::cache::{CachedResponse, ResponseCache};
use rust_scrapling::spiders::engine::CrawlerEngine;
use rust_scrapling::spiders::session::SessionManager;
use rust_scrapling::spiders::templates::ShopifySpider;
use std::collections::HashMap;
use std::sync::Arc;

const HOST: &str = "example-shop.com";

fn collections_page() -> serde_json::Value {
    serde_json::json!({
        "collections": [
            {"handle": "lipsticks", "title": "Lipsticks", "products_count": 2},
            {"handle": "empty-collection", "title": "Empty", "products_count": 0},
        ]
    })
}

fn products_page() -> serde_json::Value {
    serde_json::json!({
        "products": [
            {
                "id": 1,
                "title": "Matte Lipstick",
                "handle": "matte-lipstick",
                "vendor": "Acme",
                "body_html": "<p>A <b>matte</b> lipstick</p>",
                "images": [{"src": "https://cdn.shopify.com/lipstick.jpg"}],
                "variants": [
                    {"id": 11, "title": "Red", "sku": "LIP-RED", "price": "9.00",
                     "available": true, "compare_at_price": "12.00"},
                    {"id": 12, "title": "Nude", "sku": null, "price": "9.00",
                     "available": false, "compare_at_price": "0.00"},
                ],
            },
            {
                "id": 2,
                "title": "Brow Pencil",
                "handle": "brow-pencil",
                "vendor": "Acme",
                "body_html": null,
                "images": [],
                "variants": [
                    {"id": 21, "title": "Default Title", "sku": "BROW-1",
                     "price": "7.50", "available": true},
                ],
            },
        ]
    })
}

async fn seed(dir: &str, pages: &[(String, serde_json::Value)]) {
    let cache = ResponseCache::new(&format!("{}/cache", dir)).unwrap();
    for (url, payload) in pages {
        let cached = CachedResponse {
            status: 200,
            content_type: "application/json".to_string(),
            body: payload.to_string(),
            url: url.clone(),
            headers: HashMap::new(),
        };
        cache.put(url, &cached).await.unwrap();
    }
}

fn url(path_and_query: &str) -> String {
    format!("https://{HOST}{path_and_query}")
}

async fn crawl_store() -> Vec<serde_json::Value> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    seed(
        &base,
        &[
            (
                url("/collections.json?page=1&limit=250"),
                collections_page(),
            ),
            (
                url("/collections.json?page=2&limit=250"),
                serde_json::json!({"collections": []}),
            ),
            (
                url("/collections/lipsticks/products.json?page=1&limit=250"),
                products_page(),
            ),
            (
                url("/collections/lipsticks/products.json?page=2&limit=250"),
                serde_json::json!({"products": []}),
            ),
        ],
    )
    .await;

    let spider = ShopifySpider::builder(HOST).development_mode(true).build();
    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    engine.crawl().await.items.into_iter().collect()
}

#[tokio::test]
async fn extracts_one_item_per_variant_from_the_json_api() {
    let mut items = crawl_store().await;
    assert_eq!(items.len(), 3, "two lipstick variants + one brow pencil");
    items.sort_by_key(|i| i["identifier"].as_i64());

    // Variant with its own title: appended to the product name.
    assert_eq!(items[0]["name"], "Matte Lipstick - Red");
    assert_eq!(items[0]["price"], "9.00");
    assert_eq!(items[0]["category"], "Lipsticks");
    assert_eq!(items[0]["brand"], "Acme");
    assert_eq!(items[0]["identifier"], 11);
    assert_eq!(items[0]["sku"], "LIP-RED");
    assert_eq!(
        items[0]["stock"],
        serde_json::Value::Null,
        "available => null"
    );
    assert_eq!(
        items[0]["image_url"],
        "https://cdn.shopify.com/lipstick.jpg"
    );
    assert_eq!(
        items[0]["url"],
        "https://example-shop.com/collections/lipsticks/products/matte-lipstick"
    );
    assert_eq!(items[0]["description"], "A matte lipstick", "tags stripped");
    assert_eq!(items[0]["old_price"], "12.00");

    // Unavailable variant: stock 0; zero compare_at_price: no old_price.
    assert_eq!(items[1]["name"], "Matte Lipstick - Nude");
    assert_eq!(items[1]["stock"], 0);
    assert_eq!(items[1]["old_price"], "");
    assert_eq!(items[1]["sku"], "", "null sku becomes empty string");

    // "Default Title" variant: product name used as-is.
    assert_eq!(items[2]["name"], "Brow Pencil");
    assert_eq!(items[2]["image_url"], "", "no images");
    assert_eq!(items[2]["description"], "", "null body_html");
}

#[tokio::test]
async fn empty_collections_are_not_requested() {
    // The empty-collection products URL is absent from the cache; if the
    // spider requested it, it would count as a failed request.
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    seed(
        &base,
        &[
            (
                url("/collections.json?page=1&limit=250"),
                collections_page(),
            ),
            (
                url("/collections.json?page=2&limit=250"),
                serde_json::json!({"collections": []}),
            ),
            (
                url("/collections/lipsticks/products.json?page=1&limit=250"),
                serde_json::json!({"products": []}),
            ),
        ],
    )
    .await;

    let spider = ShopifySpider::builder(HOST).development_mode(true).build();
    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;
    assert_eq!(result.stats.failed_requests_count, 0);
    assert_eq!(
        result.stats.cache_hits, 3,
        "collections p1+p2, lipsticks p1"
    );
}

#[tokio::test]
async fn variants_seen_in_multiple_collections_are_emitted_once() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    let two_collections = serde_json::json!({
        "collections": [
            {"handle": "lipsticks", "title": "Lipsticks", "products_count": 2},
            {"handle": "all", "title": "All", "products_count": 2},
        ]
    });
    seed(
        &base,
        &[
            (url("/collections.json?page=1&limit=250"), two_collections),
            (
                url("/collections.json?page=2&limit=250"),
                serde_json::json!({"collections": []}),
            ),
            (
                url("/collections/lipsticks/products.json?page=1&limit=250"),
                products_page(),
            ),
            (
                url("/collections/lipsticks/products.json?page=2&limit=250"),
                serde_json::json!({"products": []}),
            ),
            (
                url("/collections/all/products.json?page=1&limit=250"),
                products_page(), // same products listed again
            ),
            (
                url("/collections/all/products.json?page=2&limit=250"),
                serde_json::json!({"products": []}),
            ),
        ],
    )
    .await;

    let spider = ShopifySpider::builder(HOST).development_mode(true).build();
    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;
    assert_eq!(
        result.items.len(),
        3,
        "duplicate variant ids across collections are deduplicated"
    );
}
