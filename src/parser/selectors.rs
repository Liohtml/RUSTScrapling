//! The [`Selectors`] collection: an ordered list of [`Selector`] matches
//! with batch query and extraction helpers.

use crate::core::TextHandler;
use crate::parser::selector::Selector;

/// A collection of [`Selector`] items (typically the matches of a CSS
/// query) with batch operations.
///
/// Supports indexing (`results[0]`) and iteration by value or reference.
#[derive(Debug, Clone)]
pub struct Selectors {
    items: Vec<Selector>,
}

impl Selectors {
    /// Create a new `Selectors` from a `Vec` of [`Selector`].
    pub fn new(items: Vec<Selector>) -> Self {
        Self { items }
    }

    /// Number of selectors in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the collection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return the first selector, if any.
    #[must_use]
    pub fn first(&self) -> Option<&Selector> {
        self.items.first()
    }

    /// Return the last selector, if any.
    #[must_use]
    pub fn last(&self) -> Option<&Selector> {
        self.items.last()
    }

    /// Run a CSS selector against every item and flatten the results into
    /// one collection (order: all matches of the first item, then the
    /// second, and so on).
    #[must_use]
    pub fn css(&self, selector: &str) -> Selectors {
        let items: Vec<Selector> = self
            .items
            .iter()
            .flat_map(|s| s.css(selector).into_iter())
            .collect();
        Selectors::new(items)
    }

    /// Apply a regex to every item's recursive text and collect all
    /// matches (see [`Selector::re`] for the flag semantics).
    #[must_use]
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

    /// Apply a regex across the items in order and return the first match
    /// found in any of them.
    #[must_use]
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

    /// Recursive, stripped text of the first item (see
    /// [`Selector::get_all_text`]), or `default` when the collection is
    /// empty.
    #[must_use]
    pub fn get_first(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items
            .first()
            .map(|s| s.get_all_text("", true, &[], None))
            .or(default)
    }

    /// Recursive, stripped text of every item, one entry per item.
    #[must_use]
    pub fn getall(&self) -> Vec<TextHandler> {
        self.items
            .iter()
            .map(|s| s.get_all_text("", true, &[], None))
            .collect()
    }

    /// Return the first item matching a predicate, if any.
    #[must_use]
    pub fn search<F>(&self, func: F) -> Option<&Selector>
    where
        F: Fn(&Selector) -> bool,
    {
        self.items.iter().find(|s| func(s))
    }

    /// Keep only the items matching a predicate, returning a new
    /// `Selectors` (the original is unchanged).
    #[must_use]
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
