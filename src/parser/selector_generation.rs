//! Generate CSS or XPath expressions that uniquely address an element, for
//! logging, debugging, or re-selecting an element found by other means
//! (e.g. [`Selector::find_by_text`](crate::parser::Selector::find_by_text)).

use crate::parser::selector::Selector;

/// Generate a CSS selector addressing the given element.
///
/// Path segments use `:nth-of-type(n)` whenever an element shares its tag
/// with siblings. If `full_path` is `false`, the path stops at the nearest
/// ancestor with an `id` attribute (emitted as `#id`); if `true`, the
/// complete `>`-joined path from the root is generated.
#[must_use]
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

/// Generate an XPath expression addressing the given element.
///
/// Positional predicates (`tag[n]`) are added whenever an element shares
/// its tag with siblings. If `full_path` is `false`, the path stops at the
/// nearest ancestor with an `id` attribute (emitted as `tag[@id='…']`,
/// with quoting that is safe for ids containing quotes); if `true`, the
/// complete path from the root is generated.
#[must_use]
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
                parts.push(format!(
                    "{}[@id={}]",
                    tag,
                    xpath_string_literal(id.as_str())
                ));
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
///
/// Uses the DOM `NodeId` for identity rather than comparing `outer_html`,
/// because identical sibling HTML would otherwise collapse to the same
/// position.
fn get_nth_of_type(selector: &Selector) -> usize {
    let tag = selector.tag();
    let self_id = selector.node_id();
    if let Some(parent) = selector.parent() {
        let children = parent.children();
        let same_tag: Vec<_> = children.into_iter().filter(|c| c.tag() == tag).collect();
        if same_tag.len() <= 1 {
            return 0;
        }
        for (i, child) in same_tag.iter().enumerate() {
            if child.node_id() == self_id {
                return i + 1;
            }
        }
    }
    0
}

/// Quote a string for safe embedding inside an XPath expression. Single
/// quotes are the default; double quotes are used when the value contains
/// single quotes; `concat(...)` is used when it contains both.
fn xpath_string_literal(value: &str) -> String {
    let has_single = value.contains('\'');
    let has_double = value.contains('"');
    if !has_single {
        format!("'{}'", value)
    } else if !has_double {
        format!("\"{}\"", value)
    } else {
        // Both quote types present. Split on single quotes and rejoin via
        // concat() so neither delimiter has to be escaped.
        let parts: Vec<String> = value.split('\'').map(|p| format!("'{}'", p)).collect();
        format!("concat({})", parts.join(", \"'\", "))
    }
}

#[cfg(test)]
mod tests {
    use super::xpath_string_literal;

    #[test]
    fn xpath_literal_plain() {
        assert_eq!(xpath_string_literal("foo"), "'foo'");
    }

    #[test]
    fn xpath_literal_with_single_quote_uses_double() {
        assert_eq!(xpath_string_literal("foo' or '1'='1"), "\"foo' or '1'='1\"");
    }

    #[test]
    fn xpath_literal_with_both_quotes_uses_concat() {
        // Contains both ' and ", so concat() must be used.
        let lit = xpath_string_literal("a'b\"c");
        assert_eq!(lit, "concat('a', \"'\", 'b\"c')");
    }
}
