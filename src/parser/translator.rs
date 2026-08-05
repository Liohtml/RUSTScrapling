//! Splits Parsel-style CSS queries (`h1::text`, `a::attr(href)`) into the
//! plain CSS selector and the extraction the pseudo-element asks for.

/// A CSS query decomposed into its selector part and the extraction mode
/// requested by a trailing `::text` or `::attr(name)` pseudo-element.
pub struct CssQuery {
    /// The plain CSS selector with any trailing pseudo-element removed.
    pub selector: String,
    /// `true` when the query ended in `::text` (extract recursive text).
    pub extract_text: bool,
    /// The attribute name when the query ended in `::attr(name)`.
    pub extract_attr: Option<String>,
}

/// Parse a CSS query, splitting off a trailing `::text` or `::attr(name)`
/// pseudo-element. Queries without either pseudo-element come back with the
/// (trimmed) selector unchanged and both extraction fields unset.
#[must_use]
pub fn parse_css_query(selector: &str) -> CssQuery {
    let trimmed = selector.trim();

    if let Some(base) = trimmed.strip_suffix("::text") {
        return CssQuery {
            selector: base.trim().to_string(),
            extract_text: true,
            extract_attr: None,
        };
    }

    if let Some(rest) = trimmed.strip_suffix(')') {
        if let Some(idx) = rest.rfind("::attr(") {
            let base = &rest[..idx];
            let attr_name = &rest[idx + 7..];
            return CssQuery {
                selector: base.trim().to_string(),
                extract_text: false,
                extract_attr: Some(attr_name.trim().to_string()),
            };
        }
    }

    CssQuery {
        selector: trimmed.to_string(),
        extract_text: false,
        extract_attr: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_selector_no_extraction() {
        let q = parse_css_query("div.container");
        assert_eq!(q.selector, "div.container");
        assert!(!q.extract_text);
        assert!(q.extract_attr.is_none());
    }

    #[test]
    fn test_text_pseudo_element() {
        let q = parse_css_query("h1::text");
        assert_eq!(q.selector, "h1");
        assert!(q.extract_text);
        assert!(q.extract_attr.is_none());
    }

    #[test]
    fn test_attr_pseudo_element() {
        let q = parse_css_query("a::attr(href)");
        assert_eq!(q.selector, "a");
        assert!(!q.extract_text);
        assert_eq!(q.extract_attr.as_deref(), Some("href"));
    }
}
