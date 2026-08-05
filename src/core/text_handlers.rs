//! The [`TextHandlers`] collection: a list of [`TextHandler`] values with
//! batch regex operations, mirroring Parsel's `SelectorList` text API.

use crate::core::TextHandler;

/// A list of [`TextHandler`] values with batch operations.
///
/// Supports indexing (`values[0]`) and iteration by value or reference.
#[derive(Debug, Clone, Default)]
pub struct TextHandlers {
    items: Vec<TextHandler>,
}

impl TextHandlers {
    /// Create a new `TextHandlers` from a `Vec` of [`TextHandler`].
    pub fn new(items: Vec<TextHandler>) -> Self {
        Self { items }
    }

    /// Number of values in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the collection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The first value (cloned), or `default` when the collection is empty.
    #[must_use]
    pub fn get(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items.first().cloned().or(default)
    }

    /// All values as a slice.
    #[must_use]
    pub fn getall(&self) -> &[TextHandler] {
        &self.items
    }

    /// Apply a regex to every value and flatten all matches into a new
    /// collection (see [`TextHandler::re`] for the flag semantics).
    #[must_use]
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> TextHandlers {
        let items: Vec<TextHandler> = self
            .items
            .iter()
            .flat_map(|t| t.re(pattern, replace_entities, clean_match, case_sensitive))
            .collect();
        TextHandlers::new(items)
    }

    /// Apply a regex across the values in order and return the first match
    /// found in any of them (see [`TextHandler::re`] for the flag
    /// semantics).
    #[must_use]
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        for item in &self.items {
            if let Some(m) = item.re_first(pattern, replace_entities, clean_match, case_sensitive) {
                return Some(m);
            }
        }
        None
    }
}

impl std::ops::Index<usize> for TextHandlers {
    type Output = TextHandler;
    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

impl IntoIterator for TextHandlers {
    type Item = TextHandler;
    type IntoIter = std::vec::IntoIter<TextHandler>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a TextHandlers {
    type Item = &'a TextHandler;
    type IntoIter = std::slice::Iter<'a, TextHandler>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
