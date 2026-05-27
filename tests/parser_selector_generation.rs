use rust_scrapling::parser::selector_generation::{generate_css_selector, generate_xpath_selector};
use rust_scrapling::parser::Selector;

const HTML: &str = r#"<html><body><div id="main"><ul><li class="item">First</li><li class="item">Second</li></ul><div><p>Nested</p></div></div></body></html>"#;

#[test]
fn test_css_selector_with_id_stops_at_id() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    assert_eq!(div.len(), 1);
    let css = generate_css_selector(&div[0], false);
    assert!(css.contains("#main"), "Expected '#main' in: {}", css);
}

#[test]
fn test_css_selector_for_li_includes_nth_of_type() {
    let sel = Selector::from_html(HTML);
    let li = sel.css("li.item");
    assert!(li.len() >= 2);
    // The second li should get nth-of-type since there are multiple siblings
    let css = generate_css_selector(&li[1], true);
    assert!(
        css.contains("nth-of-type"),
        "Expected 'nth-of-type' in: {}",
        css
    );
}

#[test]
fn test_xpath_selector_with_id() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    assert_eq!(div.len(), 1);
    let xpath = generate_xpath_selector(&div[0], false);
    assert!(
        xpath.contains("@id='main'"),
        "Expected \"@id='main'\" in: {}",
        xpath
    );
}

#[test]
fn test_full_css_selector_includes_body() {
    let sel = Selector::from_html(HTML);
    let div = sel.css("#main");
    assert_eq!(div.len(), 1);
    let css = generate_css_selector(&div[0], true);
    assert!(
        css.contains("body"),
        "Expected 'body' in full path: {}",
        css
    );
}
