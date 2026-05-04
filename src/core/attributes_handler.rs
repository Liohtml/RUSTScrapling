use crate::core::TextHandler;
use indexmap::IndexMap;

/// A read-only mapping of HTML element attributes.
/// All values are wrapped in TextHandler for regex/json capabilities.
#[derive(Debug, Clone)]
pub struct AttributesHandler {
    inner: IndexMap<String, TextHandler>,
}

impl AttributesHandler {
    pub fn new(map: impl IntoIterator<Item = (String, String)>) -> Self {
        let inner: IndexMap<String, TextHandler> = map
            .into_iter()
            .map(|(k, v)| (k, TextHandler::new(v)))
            .collect();
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<&TextHandler> { self.inner.get(key) }
    pub fn contains_key(&self, key: &str) -> bool { self.inner.contains_key(key) }
    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }
    pub fn keys(&self) -> impl Iterator<Item = &str> { self.inner.keys().map(|k| k.as_str()) }
    pub fn values(&self) -> impl Iterator<Item = &TextHandler> { self.inner.values() }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &TextHandler)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Search for attributes whose values match a keyword (exact or partial).
    pub fn search_values<'a>(&'a self, keyword: &'a str, partial: bool) -> impl Iterator<Item = (&'a str, &'a TextHandler)> {
        self.inner.iter().filter_map(move |(k, v)| {
            let matches = if partial { v.as_str().contains(keyword) } else { v.as_str() == keyword };
            if matches { Some((k.as_str(), v)) } else { None }
        })
    }

    /// Serialize attributes to JSON string.
    pub fn json_string(&self) -> String {
        let map: IndexMap<&str, &str> = self.inner.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        serde_json::to_string(&map).unwrap_or_default()
    }
}

impl std::ops::Index<&str> for AttributesHandler {
    type Output = TextHandler;
    fn index(&self, key: &str) -> &Self::Output { &self.inner[key] }
}
