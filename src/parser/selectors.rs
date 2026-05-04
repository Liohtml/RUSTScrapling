use crate::core::TextHandler;
use crate::parser::selector::Selector;

/// A collection of Selector items with batch operations.
#[derive(Debug, Clone)]
pub struct Selectors {
    items: Vec<Selector>,
}

impl Selectors {
    /// Create a new Selectors from a Vec of Selector.
    pub fn new(items: Vec<Selector>) -> Self {
        Self { items }
    }

    /// Number of selectors in the collection.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return the first selector, if any.
    pub fn first(&self) -> Option<&Selector> {
        self.items.first()
    }

    /// Return the last selector, if any.
    pub fn last(&self) -> Option<&Selector> {
        self.items.last()
    }

    /// Run a CSS selector across all items and flatten results.
    pub fn css(&self, selector: &str) -> Selectors {
        let items: Vec<Selector> = self
            .items
            .iter()
            .flat_map(|s| s.css(selector).into_iter())
            .collect();
        Selectors::new(items)
    }

    /// Apply regex across all items' text content.
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        self.items
            .iter()
            .flat_map(|s| s.re(pattern, replace_entities, clean_match, case_sensitive))
            .collect()
    }

    /// Apply regex across all items and return the first match.
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        for s in &self.items {
            if let Some(m) = s.re_first(pattern, replace_entities, clean_match, case_sensitive) {
                return Some(m);
            }
        }
        None
    }

    /// Get text of the first item, or a default.
    pub fn get_first(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items
            .first()
            .map(|s| s.get_all_text("", true, &[], None))
            .or(default)
    }

    /// Get text of all items.
    pub fn getall(&self) -> Vec<TextHandler> {
        self.items
            .iter()
            .map(|s| s.get_all_text("", true, &[], None))
            .collect()
    }

    /// Search for the first item matching a predicate.
    pub fn search<F>(&self, func: F) -> Option<&Selector>
    where
        F: Fn(&Selector) -> bool,
    {
        self.items.iter().find(|s| func(s))
    }

    /// Filter items by a predicate, returning a new Selectors.
    pub fn filter<F>(&self, func: F) -> Selectors
    where
        F: Fn(&Selector) -> bool,
    {
        let items: Vec<Selector> = self.items.iter().filter(|s| func(s)).cloned().collect();
        Selectors::new(items)
    }
}

impl std::ops::Index<usize> for Selectors {
    type Output = Selector;
    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

impl IntoIterator for Selectors {
    type Item = Selector;
    type IntoIter = std::vec::IntoIter<Selector>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a Selectors {
    type Item = &'a Selector;
    type IntoIter = std::slice::Iter<'a, Selector>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
