use crate::parser::selector::Selector;

/// Generate a CSS selector for the given element.
/// If `full_path` is false, stops at nearest ancestor with an `id`.
/// If `full_path` is true, generates complete path from root.
pub fn generate_css_selector(selector: &Selector, full_path: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(selector.clone());

    while let Some(el) = current {
        let tag = el.tag().to_string();
        if tag.is_empty() || tag == "html" || tag == "[document]" {
            break;
        }

        if let Some(id) = el.attrib().get("id") {
            if !full_path {
                parts.push(format!("#{}", id.as_str()));
                break;
            }
        }

        let position = get_nth_of_type(&el);
        if position > 0 {
            parts.push(format!("{}:nth-of-type({})", tag, position));
        } else {
            parts.push(tag);
        }

        current = el.parent();
    }

    parts.reverse();
    parts.join(" > ")
}

/// Generate an XPath selector for the given element.
pub fn generate_xpath_selector(selector: &Selector, full_path: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(selector.clone());

    while let Some(el) = current {
        let tag = el.tag().to_string();
        if tag.is_empty() || tag == "html" || tag == "[document]" {
            break;
        }

        if let Some(id) = el.attrib().get("id") {
            if !full_path {
                parts.push(format!("{}[@id='{}']", tag, id.as_str()));
                break;
            }
        }

        let position = get_nth_of_type(&el);
        if position > 0 {
            parts.push(format!("{}[{}]", tag, position));
        } else {
            parts.push(tag);
        }

        current = el.parent();
    }

    parts.reverse();
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("//{}", parts.join("/"))
    }
}

/// Count position of element among same-tag siblings. Returns 0 if unique.
fn get_nth_of_type(selector: &Selector) -> usize {
    let tag = selector.tag();
    if let Some(parent) = selector.parent() {
        let children = parent.children();
        let same_tag: Vec<_> = children.into_iter().filter(|c| c.tag() == tag).collect();
        if same_tag.len() <= 1 {
            return 0; // only child of this type, no disambiguation needed
        }
        // Find our position by comparing node identity via outer_html
        let our_html = selector.outer_html();
        for (i, child) in same_tag.iter().enumerate() {
            if child.outer_html().as_str() == our_html.as_str() {
                return i + 1;
            }
        }
    }
    0
}
