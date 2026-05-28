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

#[test]
fn identical_siblings_get_distinct_nth_of_type_positions() {
    // Three <li> with identical inner content. The bug compared outer_html
    // and returned position 1 for all of them. The fix uses NodeId.
    let html = r#"<html><body><ul><li>Apple</li><li>Apple</li><li>Apple</li></ul></body></html>"#;
    let sel = Selector::from_html(html);
    let items = sel.css("li");
    assert_eq!(items.len(), 3);

    let css1 = generate_css_selector(&items[0], true);
    let css2 = generate_css_selector(&items[1], true);
    let css3 = generate_css_selector(&items[2], true);
    assert!(css1.contains("nth-of-type(1)"), "got: {}", css1);
    assert!(css2.contains("nth-of-type(2)"), "got: {}", css2);
    assert!(css3.contains("nth-of-type(3)"), "got: {}", css3);

    let xp1 = generate_xpath_selector(&items[0], true);
    let xp2 = generate_xpath_selector(&items[1], true);
    let xp3 = generate_xpath_selector(&items[2], true);
    assert!(xp1.ends_with("li[1]"), "got: {}", xp1);
    assert!(xp2.ends_with("li[2]"), "got: {}", xp2);
    assert!(xp3.ends_with("li[3]"), "got: {}", xp3);
}

#[test]
fn xpath_selector_escapes_id_with_single_quote() {
    // Adversarial id containing a single quote. Without escaping the
    // generated XPath would be `div[@id='foo' or '1'='1']`, which is
    // syntactically valid but semantically attacker-controlled.
    let html = r#"<html><body><div id="foo' or '1'='1">x</div></body></html>"#;
    let sel = Selector::from_html(html);
    let div = sel.css("div");
    assert_eq!(div.len(), 1);
    let xp = generate_xpath_selector(&div[0], false);
    // Because the id contains a single quote, the generator switches to
    // double-quoted delimiters; the original `'` no longer terminates the
    // string.
    assert!(
        xp.contains("@id=\"foo' or '1'='1\""),
        "expected double-quoted id literal, got: {}",
        xp
    );
}
