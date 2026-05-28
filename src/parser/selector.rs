use crate::core::{AttributesHandler, TextHandler};
use crate::parser::selectors::Selectors;
use ego_tree::NodeId;
use regex::Regex;
use scraper::{ElementRef, Html, Node};
use std::collections::HashSet;
use std::rc::Rc;
use url::Url;

/// A wrapper around a node in an HTML tree, providing CSS selection,
/// text extraction, DOM navigation, regex, and JSON parsing.
#[derive(Debug, Clone)]
pub struct Selector {
    tree: Rc<Html>,
    node_id: NodeId,
    url: String,
}

impl Selector {
    /// Parse an HTML document string into a Selector pointing at the document root.
    pub fn from_html(html: &str) -> Self {
        let parsed = Html::parse_document(html);
        let root_id = parsed.tree.root().id();
        Self {
            tree: Rc::new(parsed),
            node_id: root_id,
            url: String::new(),
        }
    }

    /// Parse an HTML document string with a base URL for link resolution.
    pub fn from_html_with_url(html: &str, url: &str) -> Self {
        let parsed = Html::parse_document(html);
        let root_id = parsed.tree.root().id();
        Self {
            tree: Rc::new(parsed),
            node_id: root_id,
            url: url.to_string(),
        }
    }

    /// Create a child Selector sharing the same tree but pointing to a different node.
    fn child_selector(&self, node_id: NodeId) -> Self {
        Self {
            tree: Rc::clone(&self.tree),
            node_id,
            url: self.url.clone(),
        }
    }

    /// Get a NodeRef for this selector's node.
    fn node_ref(&self) -> ego_tree::NodeRef<'_, Node> {
        self.tree
            .tree
            .get(self.node_id)
            .expect("node_id should be valid")
    }

    /// Try to get an ElementRef for this node (only works for element nodes).
    fn element_ref(&self) -> Option<ElementRef<'_>> {
        ElementRef::wrap(self.node_ref())
    }

    /// Whether this selector points at the document root node.
    fn is_document_root(&self) -> bool {
        self.node_ref().value().is_document() || self.node_ref().value().is_fragment()
    }

    /// Return the tag name of this element.
    /// Returns "html" for the document root, "#text" for text nodes.
    /// Stable identity of the underlying DOM node. Useful for comparing two
    /// `Selector`s that reference the same node without resorting to HTML
    /// string equality (which is ambiguous for identical siblings).
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn tag(&self) -> &str {
        let node = self.node_ref();
        match node.value() {
            Node::Document | Node::Fragment => "html",
            Node::Text(_) => "#text",
            Node::Element(el) => el.name(),
            Node::Comment(_) => "#comment",
            Node::Doctype(_) => "#doctype",
            Node::ProcessingInstruction(_) => "#pi",
        }
    }

    /// Return the direct (non-recursive) text content of this element.
    /// Only collects immediate text children, not text inside child elements.
    pub fn text(&self) -> TextHandler {
        let node = self.node_ref();
        let mut text = String::new();
        for child in node.children() {
            if let Node::Text(ref t) = child.value() {
                text.push_str(t);
            }
        }
        TextHandler::new(text)
    }

    /// Return the element's attributes as an AttributesHandler.
    pub fn attrib(&self) -> AttributesHandler {
        if let Some(el) = self.element_ref() {
            let attrs = el
                .value()
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()));
            AttributesHandler::new(attrs)
        } else {
            AttributesHandler::new(std::iter::empty::<(String, String)>())
        }
    }

    /// Return the inner HTML of this element as a TextHandler.
    pub fn html_content(&self) -> TextHandler {
        if let Some(el) = self.element_ref() {
            TextHandler::new(el.inner_html())
        } else if self.is_document_root() {
            TextHandler::new(self.tree.html())
        } else {
            TextHandler::new("")
        }
    }

    /// Return the outer HTML of this element as a TextHandler.
    pub fn outer_html(&self) -> TextHandler {
        if let Some(el) = self.element_ref() {
            TextHandler::new(el.html())
        } else if self.is_document_root() {
            TextHandler::new(self.tree.html())
        } else {
            TextHandler::new("")
        }
    }

    /// Recursively extract text, skipping tags in `ignore_tags`.
    /// If `valid_values` is Some, only include text nodes whose trimmed content is in the set.
    pub fn get_all_text(
        &self,
        separator: &str,
        strip: bool,
        ignore_tags: &[&str],
        valid_values: Option<&[&str]>,
    ) -> TextHandler {
        let mut parts: Vec<String> = Vec::new();
        let ignore: HashSet<&str> = ignore_tags.iter().copied().collect();
        self.collect_text_recursive(self.node_ref(), &ignore, valid_values, &mut parts);

        let joined = if strip {
            let trimmed: Vec<&str> = parts
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            trimmed.join(separator)
        } else {
            parts.join(separator)
        };

        TextHandler::new(joined)
    }

    fn collect_text_recursive(
        &self,
        node: ego_tree::NodeRef<'_, Node>,
        ignore_tags: &HashSet<&str>,
        valid_values: Option<&[&str]>,
        parts: &mut Vec<String>,
    ) {
        for child in node.children() {
            match child.value() {
                Node::Text(ref t) => {
                    let s: &str = t;
                    if let Some(valid) = valid_values {
                        if valid.contains(&s.trim()) {
                            parts.push(s.to_string());
                        }
                    } else {
                        parts.push(s.to_string());
                    }
                }
                Node::Element(ref el) if !ignore_tags.contains(el.name()) => {
                    self.collect_text_recursive(child, ignore_tags, valid_values, parts);
                }
                _ => {}
            }
        }
    }

    /// Run a CSS selector query and return matching elements as Selectors.
    pub fn css(&self, selector: &str) -> Selectors {
        let css_sel = match scraper::Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => return Selectors::new(vec![]),
        };

        let items: Vec<Selector> = if self.is_document_root() {
            self.tree
                .select(&css_sel)
                .map(|el| self.child_selector(el.id()))
                .collect()
        } else if let Some(el) = self.element_ref() {
            el.select(&css_sel)
                .map(|matched| self.child_selector(matched.id()))
                .collect()
        } else {
            vec![]
        };

        Selectors::new(items)
    }

    /// Return direct element children.
    pub fn children(&self) -> Selectors {
        let node = self.node_ref();
        let items: Vec<Selector> = node
            .children()
            .filter(|child| child.value().is_element())
            .map(|child| self.child_selector(child.id()))
            .collect();
        Selectors::new(items)
    }

    /// Return the parent element, if any.
    pub fn parent(&self) -> Option<Selector> {
        let node = self.node_ref();
        node.parent().map(|p| self.child_selector(p.id()))
    }

    /// Return sibling elements (excluding self).
    pub fn siblings(&self) -> Selectors {
        let node = self.node_ref();
        let self_id = self.node_id;
        let mut items = Vec::new();

        // Get parent, iterate its children
        if let Some(parent) = node.parent() {
            for child in parent.children() {
                if child.id() != self_id && child.value().is_element() {
                    items.push(self.child_selector(child.id()));
                }
            }
        }

        Selectors::new(items)
    }

    /// Return the next sibling element.
    pub fn next(&self) -> Option<Selector> {
        let mut node = self.node_ref();
        while let Some(sibling) = node.next_sibling() {
            if sibling.value().is_element() {
                return Some(self.child_selector(sibling.id()));
            }
            node = sibling;
        }
        None
    }

    /// Return the previous sibling element.
    pub fn previous(&self) -> Option<Selector> {
        let mut node = self.node_ref();
        while let Some(sibling) = node.prev_sibling() {
            if sibling.value().is_element() {
                return Some(self.child_selector(sibling.id()));
            }
            node = sibling;
        }
        None
    }

    /// Check whether this element has a given CSS class.
    pub fn has_class(&self, class_name: &str) -> bool {
        if let Some(el) = self.element_ref() {
            el.value()
                .has_class(class_name, scraper::CaseSensitivity::CaseSensitive)
        } else {
            false
        }
    }

    /// Join a relative URL with the base URL of this selector.
    pub fn urljoin(&self, relative_url: &str) -> String {
        if self.url.is_empty() {
            return relative_url.to_string();
        }
        match Url::parse(&self.url) {
            Ok(base) => match base.join(relative_url) {
                Ok(resolved) => resolved.to_string(),
                Err(_) => relative_url.to_string(),
            },
            Err(_) => relative_url.to_string(),
        }
    }

    /// Apply a regex pattern against the full recursive text of this element.
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        let text_handler = self.get_all_text("", true, &[], None);
        text_handler.re(pattern, replace_entities, clean_match, case_sensitive)
    }

    /// Apply a regex and return only the first match.
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        let text_handler = self.get_all_text("", true, &[], None);
        text_handler.re_first(pattern, replace_entities, clean_match, case_sensitive)
    }

    /// Parse the text content of this element as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let text = self.text();
        text.json()
    }

    /// Find an element by its text content.
    pub fn find_by_text(
        &self,
        text: &str,
        first_match: bool,
        partial: bool,
        case_sensitive: bool,
    ) -> Option<Selector> {
        if first_match {
            self.find_text_recursive(self.node_ref(), text, partial, case_sensitive)
        } else {
            // Find the last match
            let all = self.find_all_by_text(text, partial, case_sensitive);
            all.into_iter().last()
        }
    }

    /// Find all elements matching the given text.
    pub fn find_all_by_text(&self, text: &str, partial: bool, case_sensitive: bool) -> Selectors {
        let mut results = Vec::new();
        self.find_all_text_recursive(self.node_ref(), text, partial, case_sensitive, &mut results);
        Selectors::new(results)
    }

    fn find_text_recursive(
        &self,
        node: ego_tree::NodeRef<'_, Node>,
        text: &str,
        partial: bool,
        case_sensitive: bool,
    ) -> Option<Selector> {
        for child in node.children() {
            if child.value().is_element() {
                // Get the direct text of this element
                let child_sel = self.child_selector(child.id());
                let el_text = child_sel.text();
                let el_str = el_text.as_str();

                if text_matches(el_str, text, partial, case_sensitive) {
                    return Some(child_sel);
                }

                // Recurse
                if let Some(found) = self.find_text_recursive(child, text, partial, case_sensitive)
                {
                    return Some(found);
                }
            }
        }
        None
    }

    fn find_all_text_recursive(
        &self,
        node: ego_tree::NodeRef<'_, Node>,
        text: &str,
        partial: bool,
        case_sensitive: bool,
        results: &mut Vec<Selector>,
    ) {
        for child in node.children() {
            if child.value().is_element() {
                let child_sel = self.child_selector(child.id());
                let el_text = child_sel.get_all_text("", true, &[], None);
                let el_str = el_text.as_str();

                if text_matches(el_str, text, partial, case_sensitive) {
                    results.push(child_sel);
                }

                self.find_all_text_recursive(child, text, partial, case_sensitive, results);
            }
        }
    }

    /// Find an element whose text matches a regex pattern.
    pub fn find_by_regex(
        &self,
        pattern: &str,
        first_match: bool,
        case_sensitive: bool,
    ) -> Option<Selector> {
        let full_pattern = if case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };
        let re = match Regex::new(&full_pattern) {
            Ok(r) => r,
            Err(_) => return None,
        };

        if first_match {
            self.find_regex_recursive(self.node_ref(), &re)
        } else {
            let mut results = Vec::new();
            self.find_all_regex_recursive(self.node_ref(), &re, &mut results);
            results.into_iter().last()
        }
    }

    fn find_regex_recursive(
        &self,
        node: ego_tree::NodeRef<'_, Node>,
        re: &Regex,
    ) -> Option<Selector> {
        for child in node.children() {
            if child.value().is_element() {
                let child_sel = self.child_selector(child.id());
                let el_text = child_sel.get_all_text("", true, &[], None);
                if re.is_match(el_text.as_str()) {
                    return Some(child_sel);
                }
                if let Some(found) = self.find_regex_recursive(child, re) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn find_all_regex_recursive(
        &self,
        node: ego_tree::NodeRef<'_, Node>,
        re: &Regex,
        results: &mut Vec<Selector>,
    ) {
        for child in node.children() {
            if child.value().is_element() {
                let child_sel = self.child_selector(child.id());
                let el_text = child_sel.get_all_text("", true, &[], None);
                if re.is_match(el_text.as_str()) {
                    results.push(child_sel);
                }
                self.find_all_regex_recursive(child, re, results);
            }
        }
    }

    /// Get the value of a specific attribute as a TextHandler.
    pub fn get_attribute(&self, key: &str) -> Option<TextHandler> {
        if let Some(el) = self.element_ref() {
            el.value().attr(key).map(TextHandler::new)
        } else {
            None
        }
    }
}

fn text_matches(haystack: &str, needle: &str, partial: bool, case_sensitive: bool) -> bool {
    if case_sensitive {
        if partial {
            haystack.contains(needle)
        } else {
            haystack.trim() == needle
        }
    } else {
        let h = haystack.to_lowercase();
        let n = needle.to_lowercase();
        if partial {
            h.contains(&n)
        } else {
            h.trim() == n
        }
    }
}
