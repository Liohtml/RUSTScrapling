//! The [`AttributesHandler`] map returned by
//! [`Selector::attrib`](crate::parser::Selector::attrib): an element's
//! attributes in document order.

use crate::core::TextHandler;
use indexmap::IndexMap;

/// A read-only mapping of HTML element attributes.
///
/// Attributes keep their document order (backed by an `IndexMap`), and all
/// values are wrapped in [`TextHandler`] for regex/JSON helpers. Indexing
/// by key (`attrs["href"]`) panics on a missing key — use
/// [`AttributesHandler::get`] for a fallible lookup.
#[derive(Debug, Clone)]
pub struct AttributesHandler {
    inner: IndexMap<String, TextHandler>,
}

impl AttributesHandler {
    /// Build a handler from `(name, value)` pairs, preserving their order.
    pub fn new(map: impl IntoIterator<Item = (String, String)>) -> Self {
        let inner: IndexMap<String, TextHandler> = map
            .into_iter()
            .map(|(k, v)| (k, TextHandler::new(v)))
            .collect();
        Self { inner }
    }

    /// Look up an attribute value by name (case-sensitive).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&TextHandler> {
        self.inner.get(key)
    }
    /// Whether an attribute with this name exists.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }
    /// Number of attributes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    /// Whether the element has no attributes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Iterate over attribute names, in document order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(|k| k.as_str())
    }
    /// Iterate over attribute values, in document order.
    pub fn values(&self) -> impl Iterator<Item = &TextHandler> {
        self.inner.values()
    }

    /// Iterate over `(name, value)` pairs, in document order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &TextHandler)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Iterate over the attributes whose value equals `keyword` — or
    /// merely contains it when `partial` is set — yielding the matching
    /// `(name, value)` pairs.
    pub fn search_values<'a>(
        &'a self,
        keyword: &'a str,
        partial: bool,
    ) -> impl Iterator<Item = (&'a str, &'a TextHandler)> {
        self.inner.iter().filter_map(move |(k, v)| {
            let matches = if partial {
                v.as_str().contains(keyword)
            } else {
                v.as_str() == keyword
            };
            if matches {
                Some((k.as_str(), v))
            } else {
                None
            }
        })
    }

    /// Serialize the attributes to a JSON object string, keys in document
    /// order (empty string on the unlikely event of a serialization error).
    #[must_use]
    pub fn json_string(&self) -> String {
        let map: IndexMap<&str, &str> = self
            .inner
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        serde_json::to_string(&map).unwrap_or_default()
    }
}

impl std::ops::Index<&str> for AttributesHandler {
    type Output = TextHandler;
    fn index(&self, key: &str) -> &Self::Output {
        &self.inner[key]
    }
}
