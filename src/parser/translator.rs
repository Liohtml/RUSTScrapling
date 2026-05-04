/// Parse CSS selector with ::text and ::attr() pseudo-element support.
pub struct CssQuery {
    pub selector: String,
    pub extract_text: bool,
    pub extract_attr: Option<String>,
}

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
