//! Adaptive element relocation, ported from upstream Scrapling's parser.
//!
//! An element's "unique properties" (tag, attributes, text, tree path,
//! parent and sibling context) are saved to [`SqliteStorage`] under an
//! identifier. When the page structure later changes and a selector stops
//! matching, [`Selector::relocate`] scores every element in the new tree
//! against the saved snapshot and returns the best matches above a
//! threshold.
//!
//! The scoring algorithm mirrors upstream Scrapling's
//! `__calculate_similarity_score`, including its later fixes: the default
//! acceptance threshold is 40% with a warning when nothing clears it
//! (upstream `333b6de`), and `css_adaptive` with `auto_save` only re-saves
//! when relocation actually found an element (upstream `cd4cdc6`).

use crate::core::storage::{SqliteStorage, StorageError};
use crate::parser::selector::Selector;
use crate::parser::selectors::Selectors;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default minimum similarity percentage for accepting a relocated element
/// (upstream fix `333b6de` raised this from 0 to 40).
pub const DEFAULT_RELOCATION_PERCENTAGE: f64 = 40.0;

/// The unique properties of an element, used to find it again after the
/// page structure changes. Mirrors upstream `_StorageTools.element_to_dict`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElementData {
    pub tag: String,
    /// Attributes with whitespace-stripped values; empty values are dropped.
    #[serde(default)]
    pub attributes: IndexMap<String, String>,
    #[serde(default)]
    pub text: Option<String>,
    /// Tag names from the root element down to this element.
    #[serde(default)]
    pub path: Vec<String>,
    #[serde(default)]
    pub parent_name: Option<String>,
    #[serde(default)]
    pub parent_attribs: IndexMap<String, String>,
    #[serde(default)]
    pub parent_text: Option<String>,
    /// Tag names of the parent's other element children.
    #[serde(default)]
    pub siblings: Vec<String>,
    /// Tag names of this element's element children.
    #[serde(default)]
    pub children: Vec<String>,
}

impl Selector {
    /// Capture this element's unique properties for adaptive relocation.
    pub fn element_data(&self) -> ElementData {
        let mut data = ElementData {
            tag: self.tag().to_string(),
            ..Default::default()
        };

        for (k, v) in self.attrib().iter() {
            let v = v.as_ref().trim();
            if !v.is_empty() {
                data.attributes.insert(k.to_string(), v.to_string());
            }
        }

        let text = self.text().as_ref().trim().to_string();
        data.text = (!text.is_empty()).then_some(text);
        data.path = self.element_path();

        if let Some(parent) = self.parent().filter(|p| p.is_element()) {
            data.parent_name = Some(parent.tag().to_string());
            data.parent_attribs = parent
                .attrib()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let parent_text = parent.text().as_ref().trim().to_string();
            data.parent_text = (!parent_text.is_empty()).then_some(parent_text);
            data.siblings = self
                .siblings()
                .into_iter()
                .map(|s| s.tag().to_string())
                .collect();
        }

        data.children = self
            .children()
            .into_iter()
            .map(|c| c.tag().to_string())
            .collect();
        data
    }

    /// Tag names from the root element down to this element.
    fn element_path(&self) -> Vec<String> {
        let mut path = vec![self.tag().to_string()];
        let mut current = self.parent();
        while let Some(node) = current {
            if !node.is_element() {
                break;
            }
            path.push(node.tag().to_string());
            current = node.parent();
        }
        path.reverse();
        path
    }

    /// Save this element's unique properties to `storage` under
    /// `identifier` for later relocation.
    pub fn save(&self, storage: &SqliteStorage, identifier: &str) -> Result<(), StorageError> {
        let value = serde_json::to_value(self.element_data())?;
        let map: HashMap<String, serde_json::Value> = match value {
            serde_json::Value::Object(obj) => obj.into_iter().collect(),
            _ => HashMap::new(),
        };
        storage.save(identifier, &map)
    }

    /// Retrieve the element properties stored under `identifier`, if any.
    pub fn retrieve(
        storage: &SqliteStorage,
        identifier: &str,
    ) -> Result<Option<ElementData>, StorageError> {
        let Some(map) = storage.retrieve(identifier)? else {
            return Ok(None);
        };
        let value = serde_json::Value::Object(map.into_iter().collect());
        Ok(Some(serde_json::from_value(value)?))
    }

    /// Search this tree again for a previously saved element. Every element
    /// is scored against `original`; the group with the highest score is
    /// returned if it reaches `percentage` (see
    /// [`DEFAULT_RELOCATION_PERCENTAGE`]), otherwise an empty collection.
    pub fn relocate(&self, original: &ElementData, percentage: f64) -> Selectors {
        let mut best_score = f64::MIN;
        let mut best: Vec<Selector> = Vec::new();

        for candidate in self.css("*") {
            let score = similarity_score(original, &candidate.element_data());
            if score > best_score {
                best_score = score;
                best = vec![candidate];
            } else if score == best_score {
                best.push(candidate);
            }
        }

        if !best.is_empty() {
            if best_score >= percentage {
                log::debug!("Highest probability was {best_score}%");
                return Selectors::new(best);
            }
            log::warn!(
                "Adaptive relocation found no element above the {percentage}% threshold \
                 (top score: {best_score}%). Lower `percentage` if this is the right element."
            );
        }
        Selectors::new(Vec::new())
    }

    /// CSS search with adaptive relocation: when `selector` matches nothing,
    /// the element previously saved under `identifier` is relocated by
    /// similarity. With `auto_save`, the first match (or the relocated
    /// element) is (re-)saved under `identifier` so future relocations track
    /// the page as it drifts.
    pub fn css_adaptive(
        &self,
        selector: &str,
        identifier: &str,
        storage: &SqliteStorage,
        auto_save: bool,
        percentage: f64,
    ) -> Result<Selectors, StorageError> {
        let identifier = if identifier.is_empty() {
            selector
        } else {
            identifier
        };

        let found = self.css(selector);
        if !found.is_empty() {
            if auto_save {
                if let Some(first) = found.first() {
                    first.save(storage, identifier)?;
                }
            }
            return Ok(found);
        }

        if let Some(original) = Self::retrieve(storage, identifier)? {
            let relocated = self.relocate(&original, percentage);
            // Guard from upstream cd4cdc6: `relocate` returns an empty
            // collection (never an error) when nothing clears the threshold,
            // so only re-save when it actually found an element.
            if auto_save {
                if let Some(first) = relocated.first() {
                    first.save(storage, identifier)?;
                }
            }
            return Ok(relocated);
        }

        Ok(found)
    }
}

/// Percentage similarity between a stored element and a candidate. Port of
/// upstream `__calculate_similarity_score`, rounded to two decimals.
pub fn similarity_score(original: &ElementData, candidate: &ElementData) -> f64 {
    let mut score = 0.0;
    let mut checks = 0u32;

    if original.tag == candidate.tag {
        score += 1.0;
    }
    checks += 1;

    if let Some(text) = &original.text {
        score += str_ratio(text, candidate.text.as_deref().unwrap_or(""));
        checks += 1;
    }

    // If both have no attributes, it still counts for something.
    score += dict_diff(&original.attributes, &candidate.attributes);
    checks += 1;

    // Separate similarity test for class, id, href, src — this helps with
    // full structural changes.
    for key in ["class", "id", "href", "src"] {
        if let Some(value) = original.attributes.get(key) {
            score += str_ratio(
                value,
                candidate
                    .attributes
                    .get(key)
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            checks += 1;
        }
    }

    score += seq_ratio(&original.path, &candidate.path);
    checks += 1;

    if let (Some(parent), Some(candidate_parent)) = (&original.parent_name, &candidate.parent_name)
    {
        score += str_ratio(parent, candidate_parent);
        checks += 1;

        score += dict_diff(&original.parent_attribs, &candidate.parent_attribs);
        checks += 1;

        if let Some(parent_text) = &original.parent_text {
            score += str_ratio(parent_text, candidate.parent_text.as_deref().unwrap_or(""));
            checks += 1;
        }
    }

    if !original.siblings.is_empty() {
        score += seq_ratio(&original.siblings, &candidate.siblings);
        checks += 1;
    }

    ((score / checks as f64) * 100.0 * 100.0).round() / 100.0
}

/// Similarity between two maps: half key similarity, half value similarity
/// (port of upstream `__calculate_dict_diff`).
fn dict_diff(a: &IndexMap<String, String>, b: &IndexMap<String, String>) -> f64 {
    let a_keys: Vec<&String> = a.keys().collect();
    let b_keys: Vec<&String> = b.keys().collect();
    let a_vals: Vec<&String> = a.values().collect();
    let b_vals: Vec<&String> = b.values().collect();
    seq_ratio(&a_keys, &b_keys) * 0.5 + seq_ratio(&a_vals, &b_vals) * 0.5
}

fn str_ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    seq_ratio(&a, &b)
}

/// Ratcliff/Obershelp similarity in `[0, 1]` for arbitrary sequences —
/// equivalent to Python's `difflib.SequenceMatcher.ratio()` (without the
/// autojunk heuristic): `2 * M / (len(a) + len(b))` where `M` is the total
/// length of matching blocks found by recursive longest-common-substring.
fn seq_ratio<T: PartialEq>(a: &[T], b: &[T]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    2.0 * matching_len(a, b) as f64 / (a.len() + b.len()) as f64
}

fn matching_len<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    // Longest common contiguous run via DP over one row.
    let mut best_len = 0;
    let mut best_a = 0;
    let mut best_b = 0;
    let mut prev = vec![0usize; b.len() + 1];
    for (i, ai) in a.iter().enumerate() {
        let mut row = vec![0usize; b.len() + 1];
        for (j, bj) in b.iter().enumerate() {
            if ai == bj {
                let run = prev[j] + 1;
                row[j + 1] = run;
                if run > best_len {
                    best_len = run;
                    best_a = i + 1 - run;
                    best_b = j + 1 - run;
                }
            }
        }
        prev = row;
    }
    if best_len == 0 {
        return 0;
    }
    best_len
        + matching_len(&a[..best_a], &b[..best_b])
        + matching_len(&a[best_a + best_len..], &b[best_b + best_len..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_ratio_matches_difflib_semantics() {
        // difflib: SequenceMatcher(None, "abcd", "bcde").ratio() == 0.75
        assert_eq!(str_ratio("abcd", "bcde"), 0.75);
        assert_eq!(str_ratio("", ""), 1.0);
        assert_eq!(str_ratio("abc", ""), 0.0);
        assert_eq!(str_ratio("same", "same"), 1.0);
    }

    #[test]
    fn identical_elements_score_100() {
        let html = r#"<html><body><div id="a" class="x">hello</div></body></html>"#;
        let sel = Selector::from_html(html);
        let el = sel.css("#a");
        let data = el.first().unwrap().element_data();
        assert_eq!(similarity_score(&data, &data), 100.0);
    }

    #[test]
    fn element_data_captures_context() {
        let html = r#"<html><body><ul id="list"><li>one</li><li id="target" class="c">two</li><li>three</li></ul></body></html>"#;
        let sel = Selector::from_html(html);
        let el = sel.css("#target");
        let data = el.first().unwrap().element_data();
        assert_eq!(data.tag, "li");
        assert_eq!(data.text.as_deref(), Some("two"));
        assert_eq!(
            data.attributes.get("id").map(String::as_str),
            Some("target")
        );
        assert_eq!(data.path, vec!["html", "body", "ul", "li"]);
        assert_eq!(data.parent_name.as_deref(), Some("ul"));
        assert_eq!(data.siblings, vec!["li", "li"]);
    }
}
