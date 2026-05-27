use rust_scrapling::core::text_handler::TextHandler;
use rust_scrapling::fetchers::response::Response;
use rust_scrapling::parser::Selector;
use std::collections::HashMap;

const ECOMMERCE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head><title>Shop - Products</title></head>
<body>
<div id="products" class="product-list">
  <div class="product" data-id="1">
    <h2 class="product-name">Laptop Pro</h2>
    <span class="price">$999.99</span>
    <p class="description">A powerful laptop for professionals.</p>
    <a href="/products/1" class="details-link">View Details</a>
  </div>
  <div class="product" data-id="2">
    <h2 class="product-name">Wireless Mouse</h2>
    <span class="price">$29.99</span>
    <p class="description">Ergonomic wireless mouse.</p>
    <a href="/products/2" class="details-link">View Details</a>
  </div>
  <div class="product" data-id="3">
    <h2 class="product-name">USB-C Hub</h2>
    <span class="price">$49.99</span>
    <p class="description">7-in-1 USB-C hub with HDMI.</p>
    <a href="/products/3" class="details-link">View Details</a>
  </div>
</div>
<nav>
  <a href="/page/2" class="next-page">Next</a>
</nav>
<script>var analytics = {};</script>
</body>
</html>
"#;

#[test]
fn test_extract_all_product_names() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let names = sel.css("h2.product-name");
    assert_eq!(names.len(), 3);

    let texts: Vec<String> = names
        .into_iter()
        .map(|n| n.text().as_str().to_string())
        .collect();
    assert_eq!(texts, vec!["Laptop Pro", "Wireless Mouse", "USB-C Hub"]);
}

#[test]
fn test_extract_prices_with_regex() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let prices = sel.css("span.price");
    assert_eq!(prices.len(), 3);

    // Use regex to extract the numeric price from each price element
    let price_values: Vec<String> = prices
        .into_iter()
        .filter_map(|p| p.re_first(r"\$(\d+\.\d+)", false, true, true))
        .map(|th| th.as_str().to_string())
        .collect();
    assert_eq!(price_values, vec!["999.99", "29.99", "49.99"]);
}

#[test]
fn test_extract_product_links_with_urljoin() {
    let sel = Selector::from_html_with_url(ECOMMERCE_HTML, "https://example.com/shop");
    let links = sel.css("a.details-link");
    assert_eq!(links.len(), 3);

    let urls: Vec<String> = links
        .into_iter()
        .map(|link| {
            let href = link.get_attribute("href").unwrap();
            link.urljoin(href.as_str())
        })
        .collect();
    assert_eq!(
        urls,
        vec![
            "https://example.com/products/1",
            "https://example.com/products/2",
            "https://example.com/products/3",
        ]
    );
}

#[test]
fn test_filter_products_by_data_id() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let products = sel.css("div.product");
    assert_eq!(products.len(), 3);

    // Filter to only data-id="2"
    let filtered = products.filter(|p| {
        p.get_attribute("data-id")
            .map(|v| v.as_str() == "2")
            .unwrap_or(false)
    });
    assert_eq!(filtered.len(), 1);

    let name = filtered.first().unwrap().css("h2.product-name");
    assert_eq!(name.first().unwrap().text().as_str(), "Wireless Mouse");
}

#[test]
fn test_dom_navigation_parent_child() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    // Find first product, get its child product-name
    let product = sel.css("div.product").first().unwrap().clone();
    let children = product.children();
    // Children should include h2, span, p, a
    assert!(children.len() >= 4);

    let name = product.css("h2.product-name");
    assert_eq!(name.first().unwrap().text().as_str(), "Laptop Pro");

    // Navigate from h2 back to parent
    let h2 = name.first().unwrap();
    let parent = h2.parent().unwrap();
    assert!(parent.has_class("product"));
}

#[test]
fn test_pagination_link_urljoin() {
    let sel = Selector::from_html_with_url(ECOMMERCE_HTML, "https://example.com/shop");
    let next_link = sel.css("a.next-page");
    assert_eq!(next_link.len(), 1);

    let href = next_link.first().unwrap().get_attribute("href").unwrap();
    let full_url = next_link.first().unwrap().urljoin(href.as_str());
    assert_eq!(full_url, "https://example.com/page/2");
}

#[test]
fn test_get_all_text_ignoring_script() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let text = sel.get_all_text(" ", true, &["script"], None);
    let text_str = text.as_str();

    // Should contain product names
    assert!(text_str.contains("Laptop Pro"));
    assert!(text_str.contains("Wireless Mouse"));
    assert!(text_str.contains("USB-C Hub"));

    // Should NOT contain script content
    assert!(!text_str.contains("analytics"));
}

#[test]
fn test_find_by_text_exact() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let found = sel.find_by_text("Wireless Mouse", true, false, true);
    assert!(found.is_some());
    let el = found.unwrap();
    assert_eq!(el.tag(), "h2");
    assert!(el.has_class("product-name"));
}

#[test]
fn test_find_by_text_partial() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let found = sel.find_by_text("USB-C", true, true, true);
    assert!(found.is_some());
}

#[test]
fn test_text_handler_chaining() {
    let th = TextHandler::new("  Hello World  ");
    let result = th.strip().to_lowercase().replace_str("world", "rust");
    assert_eq!(result.as_str(), "hello rust");
}

#[test]
fn test_text_handler_methods() {
    let th = TextHandler::new("Hello World");
    assert!(!th.is_empty());
    assert_eq!(th.len(), 11);
    assert!(th.contains_str("World"));
    assert!(th.starts_with_str("Hello"));
    assert!(th.ends_with_str("World"));

    let parts = th.split_str(" ");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].as_str(), "Hello");
    assert_eq!(parts[1].as_str(), "World");
}

#[test]
fn test_complex_css_descendant_selector() {
    let html = r#"
    <div class="a">
      <div class="b">
        <span>Deep Nested</span>
      </div>
      <span>Direct Child</span>
    </div>
    "#;
    let sel = Selector::from_html(html);

    // Descendant selector: div.a div.b span
    let deep = sel.css("div.a div.b span");
    assert_eq!(deep.len(), 1);
    assert_eq!(deep.first().unwrap().text().as_str(), "Deep Nested");

    // Child selector: div.a > span (only direct children)
    let direct = sel.css("div.a > span");
    assert_eq!(direct.len(), 1);
    assert_eq!(direct.first().unwrap().text().as_str(), "Direct Child");
}

#[test]
fn test_empty_results() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let nothing = sel.css("div.nonexistent-class");
    assert_eq!(nothing.len(), 0);
    assert!(nothing.is_empty());
    assert!(nothing.first().is_none());
}

#[test]
fn test_response_struct_integration() {
    let response = Response::new(
        200,
        "text/html".to_string(),
        ECOMMERCE_HTML.to_string(),
        "https://example.com/shop".to_string(),
        HashMap::new(),
    );

    assert!(response.is_success());
    assert_eq!(response.status(), 200);
    assert_eq!(response.url(), "https://example.com/shop");
    assert!(response.content_length() > 0);

    // Get selector from response and query
    let sel = response.selector();
    let products = sel.css("div.product");
    assert_eq!(products.len(), 3);

    let first_name = sel.css("h2.product-name").first().unwrap().text();
    assert_eq!(first_name.as_str(), "Laptop Pro");

    // urljoin should work since we passed a URL
    let link = sel.css("a.details-link").first().unwrap().clone();
    let href = link.get_attribute("href").unwrap();
    let full_url = link.urljoin(href.as_str());
    assert_eq!(full_url, "https://example.com/products/1");
}

#[test]
fn test_selectors_getall() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let names = sel.css("h2.product-name");
    let all_text: Vec<String> = names
        .getall()
        .iter()
        .map(|t| t.as_str().to_string())
        .collect();
    assert_eq!(all_text, vec!["Laptop Pro", "Wireless Mouse", "USB-C Hub"]);
}

#[test]
fn test_selectors_re_across_all() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let prices = sel.css("span.price");
    let matches = prices.re(r"\$(\d+\.\d+)", false, true, true);
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].as_str(), "999.99");
}

#[test]
fn test_sibling_navigation() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let first_product = sel.css("div.product").first().unwrap().clone();
    let siblings = first_product.siblings();
    // The other 2 products should be siblings
    assert_eq!(siblings.len(), 2);
}

#[test]
fn test_next_previous_navigation() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let products: Vec<_> = sel.css("div.product").into_iter().collect();
    assert_eq!(products.len(), 3);

    // First product's next should be second product
    let next = products[0].next().unwrap();
    let next_name = next.css("h2.product-name").first().unwrap().text();
    assert_eq!(next_name.as_str(), "Wireless Mouse");

    // Second product's previous should be first product
    let prev = products[1].previous().unwrap();
    let prev_name = prev.css("h2.product-name").first().unwrap().text();
    assert_eq!(prev_name.as_str(), "Laptop Pro");
}

#[test]
fn test_html_content_and_outer_html() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let h2 = sel.css("h2.product-name").first().unwrap().clone();

    let inner = h2.html_content();
    assert_eq!(inner.as_str(), "Laptop Pro");

    let outer = h2.outer_html();
    assert!(outer.as_str().contains("<h2"));
    assert!(outer.as_str().contains("Laptop Pro"));
    assert!(outer.as_str().contains("</h2>"));
}

#[test]
fn test_has_class() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let products = sel.css("div.product");
    let product = products.first().unwrap();
    assert!(product.has_class("product"));
    assert!(!product.has_class("nonexistent"));
}

#[test]
fn test_tag_name() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let names = sel.css("h2.product-name");
    let h2 = names.first().unwrap();
    assert_eq!(h2.tag(), "h2");

    let products = sel.css("div.product");
    let div = products.first().unwrap();
    assert_eq!(div.tag(), "div");
}

#[test]
fn test_text_handler_clean() {
    let th = TextHandler::new("  Hello\t\n  World  ");
    let cleaned = th.clean(false);
    assert_eq!(cleaned.as_str(), "Hello World");
}

#[test]
fn test_text_handler_json() {
    let th = TextHandler::new(r#"{"name": "test", "value": 42}"#);
    let json = th.json().unwrap();
    assert_eq!(json["name"], "test");
    assert_eq!(json["value"], 42);
}

#[test]
fn test_find_all_by_text() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    // "View Details" appears 3 times
    let found = sel.find_all_by_text("View Details", false, true);
    assert_eq!(found.len(), 3);
}

#[test]
fn test_urljoin_without_base_url() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    // Without base URL, urljoin returns the relative URL as-is
    let result = sel.urljoin("/products/1");
    assert_eq!(result, "/products/1");
}

#[test]
fn test_attribute_handler_from_element() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let products = sel.css("div.product");
    let product = products.first().unwrap();
    let attribs = product.attrib();
    let data_id = attribs.get("data-id");
    assert!(data_id.is_some());
    assert_eq!(data_id.unwrap().as_str(), "1");
}

#[test]
fn test_index_into_selectors() {
    let sel = Selector::from_html(ECOMMERCE_HTML);
    let products = sel.css("div.product");
    // Index access
    let second = &products[1];
    let name = second.css("h2.product-name").first().unwrap().text();
    assert_eq!(name.as_str(), "Wireless Mouse");
}

#[test]
fn test_response_not_success() {
    let response = Response::new(
        404,
        "text/html".to_string(),
        "<html><body>Not Found</body></html>".to_string(),
        "https://example.com/missing".to_string(),
        HashMap::new(),
    );
    assert!(!response.is_success());
    assert_eq!(response.status(), 404);
}
