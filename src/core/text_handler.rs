use regex::Regex;
use std::fmt;

/// A string wrapper with scraping-specific methods (regex, JSON, cleaning).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TextHandler {
    value: String,
}

impl TextHandler {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn into_string(self) -> String {
        self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn strip(&self) -> TextHandler {
        TextHandler::new(self.value.trim())
    }

    pub fn to_lowercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_lowercase())
    }

    pub fn to_uppercase(&self) -> TextHandler {
        TextHandler::new(self.value.to_uppercase())
    }

    pub fn contains_str(&self, pattern: &str) -> bool {
        self.value.contains(pattern)
    }

    pub fn replace_str(&self, from: &str, to: &str) -> TextHandler {
        TextHandler::new(self.value.replace(from, to))
    }

    pub fn starts_with_str(&self, prefix: &str) -> bool {
        self.value.starts_with(prefix)
    }

    pub fn ends_with_str(&self, suffix: &str) -> bool {
        self.value.ends_with(suffix)
    }

    pub fn split_str(&self, delimiter: &str) -> Vec<TextHandler> {
        self.value.split(delimiter).map(TextHandler::new).collect()
    }

    /// Clean whitespace: remove tabs, CR, LF, collapse spaces. Optionally decode HTML entities.
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

    /// Parse text as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.value)
    }

    /// Apply regex. Returns captured group 1 if present, else group 0.
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

    pub fn get(&self) -> &TextHandler {
        self
    }

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
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}
