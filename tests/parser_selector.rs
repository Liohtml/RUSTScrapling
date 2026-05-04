use rust_scrapling::parser::{Selector, Selectors};

const HTML: &str = r#"
<html>
<head><title>Test Page</title></head>
<body>
  <div id="main" class="container">
    <h1 class="title">Hello World</h1>
    <p class="description">This is a <strong>test</strong> page.</p>
    <ul id="items">
      <li class="item" data-price="19.99">Item 1</li>
      <li class="item" data-price="29.99">Item 2</li>
      <li class="item special" data-price="39.99">Item 3</li>
    </ul>
    <a href="/next" class="nav-link">Next Page</a>
    <script>var x = 1;</script>
  </div>
</body>
</html>
"#;

#[test]
fn test_from_html_creates_valid_selector() {
    let sel = Selector::from_html(HTML);
    assert_eq!(sel.tag(), "html");
}

#[test]
fn test_css_h1_title() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("h1.title");
    assert_eq!(results.len(), 1);
    let h1 = &results[0];
    assert_eq!(h1.text().as_str(), "Hello World");
}

#[test]
fn test_css_li_item() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    assert_eq!(items.len(), 3);
}

#[test]
fn test_css_id_main() {
    let sel = Selector::from_html(HTML);
    let divs = sel.css("#main");
    assert_eq!(divs.len(), 1);
    assert_eq!(divs[0].tag(), "div");
}

#[test]
fn test_css_data_attribute() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("[data-price]");
    assert_eq!(results.len(), 3);
}

#[test]
fn test_tag_returns_correct_names() {
    let sel = Selector::from_html(HTML);
    let h1 = sel.css("h1");
    assert_eq!(h1[0].tag(), "h1");
    let ul = sel.css("ul");
    assert_eq!(ul[0].tag(), "ul");
    let a = sel.css("a");
    assert_eq!(a[0].tag(), "a");
}

#[test]
fn test_text_returns_direct_text() {
    let sel = Selector::from_html(HTML);
    let p = sel.css("p.description");
    assert_eq!(p.len(), 1);
    // Direct text of <p> is "This is a " and " page." (not "test" which is in <strong>)
    let text = p[0].text();
    assert!(text.as_str().contains("This is a"));
    assert!(text.as_str().contains("page."));
    assert!(!text.as_str().contains("test"));
}

#[test]
fn test_attrib_returns_attributes() {
    let sel = Selector::from_html(HTML);
    let a = sel.css("a.nav-link");
    assert_eq!(a.len(), 1);
    let attrs = a[0].attrib();
    assert_eq!(attrs.get("href").unwrap().as_str(), "/next");
    assert_eq!(attrs.get("class").unwrap().as_str(), "nav-link");
}

#[test]
fn test_html_content_includes_inner_tags() {
    let sel = Selector::from_html(HTML);
    let p = sel.css("p.description");
    let inner = p[0].html_content();
    assert!(inner.as_str().contains("<strong>test</strong>"));
    assert!(inner.as_str().contains("This is a"));
}

#[test]
fn test_get_all_text_ignores_script() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    assert_eq!(div.len(), 1);
    let text = div[0].get_all_text(" ", true, &["script"], None);
    assert!(text.as_str().contains("Hello World"));
    assert!(text.as_str().contains("Item 1"));
    assert!(!text.as_str().contains("var x"));
}

#[test]
fn test_children_of_ul() {
    let sel = Selector::from_html(HTML);
    let ul = sel.css("ul#items");
    assert_eq!(ul.len(), 1);
    let children = ul[0].children();
    assert_eq!(children.len(), 3);
    for child in &children {
        assert_eq!(child.tag(), "li");
    }
}

#[test]
fn test_parent_of_li() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.item");
    let parent = li[0].parent().unwrap();
    assert_eq!(parent.tag(), "ul");
}

#[test]
fn test_has_class_positive_and_negative() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.item");
    assert!(li[0].has_class("item"));
    assert!(!li[0].has_class("special"));
    assert!(li[2].has_class("special"));
    assert!(li[2].has_class("item"));
}

#[test]
fn test_selectors_len_first_last() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    assert_eq!(items.len(), 3);
    assert!(!items.is_empty());
    assert_eq!(items.first().unwrap().text().as_str(), "Item 1");
    assert_eq!(items.last().unwrap().text().as_str(), "Item 3");
}

#[test]
fn test_selectors_filter() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let special = items.filter(|s| s.has_class("special"));
    assert_eq!(special.len(), 1);
    assert_eq!(special[0].text().as_str(), "Item 3");
}

#[test]
fn test_selectors_search() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let found = items.search(|s| s.text().as_str().contains("Item 2"));
    assert!(found.is_some());
    assert_eq!(found.unwrap().text().as_str(), "Item 2");
}

#[test]
fn test_urljoin_with_base_url() {
    let sel = Selector::from_html_with_url(HTML, "https://example.com/page/1");
    let a = sel.css("a.nav-link");
    let href = a[0].get_attribute("href").unwrap();
    let full_url = a[0].urljoin(href.as_str());
    assert_eq!(full_url, "https://example.com/next");
}

#[test]
fn test_find_by_text_exact() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_text("Hello World", true, false, true);
    assert!(found.is_some());
    assert_eq!(found.unwrap().tag(), "h1");
}

#[test]
fn test_find_by_text_partial() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_text("Hello", true, true, true);
    assert!(found.is_some());
    assert_eq!(found.unwrap().tag(), "h1");
}

#[test]
fn test_find_by_regex() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_by_regex(r"Item \d+", true, true);
    assert!(found.is_some());
    // First element whose recursive text matches is the <html> element (it contains all text)
    // so let's search within a narrower scope
    let ul = sel.css("ul#items");
    let found = ul[0].find_by_regex(r"^Item 2$", true, true);
    assert!(found.is_some());
    assert_eq!(found.unwrap().tag(), "li");
}

#[test]
fn test_json_on_script() {
    let json_html = r#"<html><body><script>{"key": "value"}</script></body></html>"#;
    let sel = Selector::from_html(json_html);
    let script = sel.css("script");
    assert_eq!(script.len(), 1);
    let val = script[0].json().unwrap();
    assert_eq!(val["key"], "value");
}

#[test]
fn test_empty_css_result() {
    let sel = Selector::from_html(HTML);
    let results = sel.css("div.nonexistent");
    assert_eq!(results.len(), 0);
    assert!(results.is_empty());
}

#[test]
fn test_outer_html() {
    let sel = Selector::from_html(HTML);
    let h1 = sel.css("h1.title");
    let outer = h1[0].outer_html();
    assert!(outer.as_str().contains("<h1"));
    assert!(outer.as_str().contains("Hello World"));
    assert!(outer.as_str().contains("</h1>"));
}

#[test]
fn test_next_sibling() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let next = items[0].next();
    assert!(next.is_some());
    assert_eq!(next.unwrap().text().as_str(), "Item 2");
}

#[test]
fn test_previous_sibling() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let prev = items[1].previous();
    assert!(prev.is_some());
    assert_eq!(prev.unwrap().text().as_str(), "Item 1");
}

#[test]
fn test_siblings() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let siblings = items[0].siblings();
    assert_eq!(siblings.len(), 2);
}

#[test]
fn test_get_attribute() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.item");
    let price = li[0].get_attribute("data-price");
    assert!(price.is_some());
    assert_eq!(price.unwrap().as_str(), "19.99");

    let missing = li[0].get_attribute("nonexistent");
    assert!(missing.is_none());
}

#[test]
fn test_selectors_getall() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let all = items.getall();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].as_str(), "Item 1");
    assert_eq!(all[1].as_str(), "Item 2");
    assert_eq!(all[2].as_str(), "Item 3");
}

#[test]
fn test_selectors_get_first() {
    let sel = Selector::from_html(HTML);
    let items = sel.css("li.item");
    let first = items.get_first(None);
    assert!(first.is_some());
    assert_eq!(first.unwrap().as_str(), "Item 1");

    let empty = sel.css("div.nonexistent");
    let fallback =
        empty.get_first(Some(rust_scrapling::core::TextHandler::new("default")));
    assert_eq!(fallback.unwrap().as_str(), "default");
}

#[test]
fn test_css_from_element_level() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    // CSS from an element (not document root) should work
    let h1 = div[0].css("h1");
    assert_eq!(h1.len(), 1);
    assert_eq!(h1[0].text().as_str(), "Hello World");
}

#[test]
fn test_find_all_by_text() {
    let sel = Selector::from_html(HTML);
    let found = sel.find_all_by_text("Item", true, true);
    // Should find at least the 3 li elements (and potentially the ul that contains them)
    assert!(found.len() >= 3);
}
