//! The [`TextHandler`] string wrapper returned by every text-extraction
//! method in the crate, with regex, JSON, and cleaning helpers.

use regex::Regex;
use std::fmt;

/// A string wrapper with scraping-specific methods (regex, JSON, cleaning).
///
/// All transforming methods ([`TextHandler::strip`], [`TextHandler::clean`],
/// …) return a new `TextHandler` and leave the original untouched. Use
/// [`TextHandler::as_str`] / [`TextHandler::into_string`] (or the
/// `Display` / `AsRef<str>` impls) to get at the plain string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TextHandler {
    value: String,
}

impl TextHandler {
    /// Wrap a string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// The wrapped text as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Consume the handler and return the owned `String`.
    #[must_use]
    pub fn into_string(self) -> String {
        self.value
    }

    /// Whether the wrapped text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Length of the wrapped text in *bytes* (like [`str::len`]), not
    /// characters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Return a copy with leading and trailing whitespace removed
    /// (Python-style name for [`str::trim`]).
    #[must_use]
    pub fn strip(&self) -> TextHandler {
        TextHandler::new(self.value.trim())
    }

    /// Return a lowercased copy.
    #[must_use]
    pub fn to_lowercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_lowercase())
    }

    /// Return an uppercased copy.
    #[must_use]
    pub fn to_uppercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_uppercase())
    }

    /// Whether the text contains the given substring.
    #[must_use]
    pub fn contains_str(&self, pattern: &str) -> bool {
        self.value.contains(pattern)
    }

    /// Return a copy with every occurrence of `from` replaced by `to`.
    #[must_use]
    pub fn replace_str(&self, from: &str, to: &str) -> TextHandler {
        TextHandler::new(self.value.replace(from, to))
    }

    /// Whether the text starts with the given prefix.
    #[must_use]
    pub fn starts_with_str(&self, prefix: &str) -> bool {
        self.value.starts_with(prefix)
    }

    /// Whether the text ends with the given suffix.
    #[must_use]
    pub fn ends_with_str(&self, suffix: &str) -> bool {
        self.value.ends_with(suffix)
    }

    /// Split the text on a delimiter, wrapping each piece (empty pieces
    /// included, like [`str::split`]).
    #[must_use]
    pub fn split_str(&self, delimiter: &str) -> Vec<TextHandler> {
        self.value.split(delimiter).map(TextHandler::new).collect()
    }

    /// Normalize whitespace: tabs and line feeds become spaces, carriage
    /// returns are removed, runs of spaces collapse to one, and the result
    /// is trimmed. With `remove_entities`, a fixed set of common HTML
    /// entities (`&lt;` `&gt;` `&quot;` `&#39;` `&apos;` `&nbsp;` `&amp;`)
    /// is decoded afterwards.
    #[must_use]
    pub fn clean(&self, remove_entities: bool) -> TextHandler {
        let mut s = self.value.replace('\t', " ");
        s = s.replace('\r', "");
        s = s.replace('\n', " ");
        while s.contains("  ") {
            s = s.replace("  ", " ");
        }
        s = s.trim().to_string();
        if remove_entities {
            s = decode_html_entities(&s);
        }
        TextHandler::new(s)
    }

    /// Parse the text as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.value)
    }

    /// Apply a regex and return all matches.
    ///
    /// For each match, capture group 1 is returned when the pattern
    /// defines capture groups, otherwise the whole match. Flags:
    /// `replace_entities` decodes common HTML entities *before* matching,
    /// `clean_match` trims each returned match, and `case_sensitive =
    /// false` prepends `(?i)` to the pattern. An invalid pattern yields an
    /// empty `Vec`.
    #[must_use]
    pub fn re(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Vec<TextHandler> {
        let text = if replace_entities {
            decode_html_entities(&self.value)
        } else {
            self.value.clone()
        };
        let full_pattern = if case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };
        let re = match Regex::new(&full_pattern) {
            Ok(r) => r,
            Err(_) => return vec![],
        };
        let mut results = Vec::new();
        for caps in re.captures_iter(&text) {
            let matched = if caps.len() > 1 {
                caps.get(1).map(|m| m.as_str())
            } else {
                caps.get(0).map(|m| m.as_str())
            };
            if let Some(s) = matched {
                let val = if clean_match { s.trim() } else { s };
                results.push(TextHandler::new(val));
            }
        }
        results
    }

    /// Apply a regex and return only the first match (see
    /// [`TextHandler::re`] for the flag semantics).
    #[must_use]
    pub fn re_first(
        &self,
        pattern: &str,
        replace_entities: bool,
        clean_match: bool,
        case_sensitive: bool,
    ) -> Option<TextHandler> {
        self.re(pattern, replace_entities, clean_match, case_sensitive)
            .into_iter()
            .next()
    }

    /// Parsel-parity accessor: returns `self` (a single value's `.get()`
    /// is itself).
    #[must_use]
    pub fn get(&self) -> &TextHandler {
        self
    }

    /// Parsel-parity accessor: returns the value as a one-element `Vec`.
    #[must_use]
    pub fn getall(&self) -> Vec<TextHandler> {
        vec![self.clone()]
    }
}

impl fmt::Display for TextHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<str> for TextHandler {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl From<String> for TextHandler {
    fn from(s: String) -> Self {
        TextHandler::new(s)
    }
}

impl From<&str> for TextHandler {
    fn from(s: &str) -> Self {
        TextHandler::new(s)
    }
}

fn decode_html_entities(s: &str) -> String {
    // `&amp;` must decode LAST: decoding it first would turn a literal
    // `&amp;lt;` (which should read as the text `&lt;`) into `<`.
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}
