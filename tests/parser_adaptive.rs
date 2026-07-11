//! Integration tests for adaptive element relocation: save/retrieve via
//! SQLite storage and similarity-based relocation after page changes.

use rust_scrapling::core::storage::SqliteStorage;
use rust_scrapling::parser::{Selector, DEFAULT_RELOCATION_PERCENTAGE};
use tempfile::tempdir;

fn storage(dir: &tempfile::TempDir) -> SqliteStorage {
    let db = dir.path().join("elements.db");
    SqliteStorage::new(db.to_str().unwrap(), "https://example.com").unwrap()
}

const ORIGINAL_PAGE: &str = r#"<html><body>
    <div id="products">
        <article class="product" data-sku="42"><h2>Widget</h2><span class="price">9.99</span></article>
    </div>
</body></html>"#;

// Same product element, but the page was restructured: the wrapper div is
// gone, the section is renamed, and the id-based selector no longer works.
const CHANGED_PAGE: &str = r#"<html><body>
    <header><nav><a href="/">home</a></nav></header>
    <section class="catalog">
        <article class="product" data-sku="42"><h2>Widget</h2><span class="price">10.99</span></article>
        <aside class="ad">buy now</aside>
    </section>
</body></html>"#;

#[test]
fn save_and_retrieve_roundtrip() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let page = Selector::from_html(ORIGINAL_PAGE);
    let product = page.css("#products .product");
    product
        .first()
        .unwrap()
        .save(&store, "product-card")
        .unwrap();

    let restored = Selector::retrieve(&store, "product-card")
        .unwrap()
        .expect("saved element data should be retrievable");
    assert_eq!(restored.tag, "article");
    assert_eq!(
        restored.attributes.get("data-sku").map(String::as_str),
        Some("42")
    );
    assert_eq!(restored.parent_name.as_deref(), Some("div"));
}

#[test]
fn relocate_finds_element_after_structure_change() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let old_page = Selector::from_html(ORIGINAL_PAGE);
    let product = old_page.css("#products .product");
    product
        .first()
        .unwrap()
        .save(&store, "product-card")
        .unwrap();

    // The old selector fails on the new page…
    let new_page = Selector::from_html(CHANGED_PAGE);
    assert!(new_page.css("#products .product").is_empty());

    // …but relocation finds the moved element by similarity.
    let saved = Selector::retrieve(&store, "product-card").unwrap().unwrap();
    let relocated = new_page.relocate(&saved, DEFAULT_RELOCATION_PERCENTAGE);
    assert_eq!(relocated.len(), 1);
    let found = relocated.first().unwrap();
    assert_eq!(found.tag(), "article");
    assert_eq!(
        found.get_attribute("data-sku").map(|v| v.to_string()),
        Some("42".to_string())
    );
}

#[test]
fn relocate_returns_empty_below_threshold() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let old_page = Selector::from_html(ORIGINAL_PAGE);
    let product = old_page.css("#products .product");
    product
        .first()
        .unwrap()
        .save(&store, "product-card")
        .unwrap();

    let saved = Selector::retrieve(&store, "product-card").unwrap().unwrap();
    let new_page = Selector::from_html(CHANGED_PAGE);
    // An impossible threshold yields no results instead of a bad match.
    let relocated = new_page.relocate(&saved, 101.0);
    assert!(relocated.is_empty());
}

#[test]
fn css_adaptive_returns_matches_and_saves_when_selector_works() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let page = Selector::from_html(ORIGINAL_PAGE);
    let found = page
        .css_adaptive(
            "#products .product",
            "product-card",
            &store,
            true,
            DEFAULT_RELOCATION_PERCENTAGE,
        )
        .unwrap();
    assert_eq!(found.len(), 1);

    // auto_save persisted the element under the identifier.
    assert!(Selector::retrieve(&store, "product-card")
        .unwrap()
        .is_some());
}

#[test]
fn css_adaptive_relocates_when_selector_fails() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    // First crawl: selector works, element is auto-saved.
    let old_page = Selector::from_html(ORIGINAL_PAGE);
    old_page
        .css_adaptive(
            "#products .product",
            "product-card",
            &store,
            true,
            DEFAULT_RELOCATION_PERCENTAGE,
        )
        .unwrap();

    // Second crawl: page restructured, selector fails, relocation kicks in.
    let new_page = Selector::from_html(CHANGED_PAGE);
    let found = new_page
        .css_adaptive(
            "#products .product",
            "product-card",
            &store,
            true,
            DEFAULT_RELOCATION_PERCENTAGE,
        )
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found.first().unwrap().tag(), "article");

    // The relocated element was re-saved: it now reflects the new page
    // (parent is the section, not the old div).
    let resaved = Selector::retrieve(&store, "product-card").unwrap().unwrap();
    assert_eq!(resaved.parent_name.as_deref(), Some("section"));
}

#[test]
fn css_adaptive_with_auto_save_survives_failed_relocation() {
    // Regression for upstream cd4cdc6: when relocation finds nothing above
    // the threshold, auto_save must not panic on the empty result.
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let old_page = Selector::from_html(ORIGINAL_PAGE);
    let product = old_page.css("#products .product");
    product
        .first()
        .unwrap()
        .save(&store, "product-card")
        .unwrap();

    let unrelated = Selector::from_html("<html><body><p>totally different</p></body></html>");
    let found = unrelated
        .css_adaptive(
            "#products .product",
            "product-card",
            &store,
            true,
            100.0, // nothing on this page scores 100 against the product
        )
        .unwrap();
    assert!(found.is_empty(), "no element clears the threshold");
}

#[test]
fn css_adaptive_without_stored_data_returns_empty() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);
    let page = Selector::from_html(CHANGED_PAGE);
    let found = page
        .css_adaptive(
            "#missing",
            "never-saved",
            &store,
            false,
            DEFAULT_RELOCATION_PERCENTAGE,
        )
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn identifier_defaults_to_selector_when_empty() {
    let dir = tempdir().unwrap();
    let store = storage(&dir);

    let page = Selector::from_html(ORIGINAL_PAGE);
    page.css_adaptive(".product", "", &store, true, DEFAULT_RELOCATION_PERCENTAGE)
        .unwrap();
    // Stored under the selector itself.
    assert!(Selector::retrieve(&store, ".product").unwrap().is_some());
}
