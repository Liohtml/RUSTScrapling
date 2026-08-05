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
//!
//! One known divergence from the Python original: element text is captured
//! as the concatenation of *all* direct text children ([`Selector::text`]),
//! while lxml's `element.text` is only the text before the first child
//! element. Scores are self-consistent within this port (saved data and
//! candidates use the same capture), but are not numerically comparable to
//! Python Scrapling's for mixed-content elements.

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
    /// The element's tag name.
    pub tag: String,
    /// Attributes with whitespace-stripped values; empty values are dropped.
    #[serde(default)]
    pub attributes: IndexMap<String, String>,
    /// Trimmed direct text content, `None` when empty.
    #[serde(default)]
    pub text: Option<String>,
    /// Tag names from the root element down to this element.
    #[serde(default)]
    pub path: Vec<String>,
    /// The parent element's tag name, if the parent is an element.
    #[serde(default)]
    pub parent_name: Option<String>,
    /// The parent element's attributes (values unstripped).
    #[serde(default)]
    pub parent_attribs: IndexMap<String, String>,
    /// The parent's trimmed direct text content, `None` when empty.
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
    #[must_use]
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
    #[must_use]
    pub fn relocate(&self, original: &ElementData, percentage: f64) -> Selectors {
        let mut best_score = f64::MIN;
        let mut best: Vec<Selector> = Vec::new();

        for candidate in self.css("*") {
            // Match upstream's `.//*` semantics: the search never returns
            // the element it starts from, and never the top-level <html>
            // element (whose parent is the document root, not an element).
            if candidate.node_id() == self.node_id()
                || !candidate.parent().map(|p| p.is_element()).unwrap_or(false)
            {
                continue;
            }
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

/// Percentage similarity (`0.0..=100.0`) between a stored element and a
/// candidate. Port of upstream `__calculate_similarity_score`, rounded to
/// two decimals: the mean of per-feature similarity checks (tag, text,
/// attributes with extra weight on `class`/`id`/`href`/`src`, tree path,
/// parent context, siblings), where only features present in `original`
/// are checked.
#[must_use]
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
/// equivalent to Python's `difflib.SequenceMatcher.ratio()` including the
/// autojunk heuristic: `2 * M / (len(a) + len(b))` where `M` is the total
/// length of matching blocks found by recursive longest-common-substring.
///
/// Like difflib, an index of positions per element of `b` is built so only
/// matching positions do work, and for `len(b) >= 200` "popular" elements
/// (frequency above 1%) cannot *seed* matches. Both are essential on real
/// pages: without them, scoring a candidate row of a large table against a
/// saved row degenerates into an O(rows²) dynamic program per candidate —
/// cubic overall — and uniform sequences (2000 identical `<tr>` siblings)
/// score ~1.0 where difflib scores ~0.0.
fn seq_ratio<T: Eq + std::hash::Hash>(a: &[T], b: &[T]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let matcher = SequenceMatcher::new(a, b);
    2.0 * matcher.matching_len(0, a.len(), 0, b.len()) as f64 / (a.len() + b.len()) as f64
}

/// Minimal port of `difflib.SequenceMatcher`'s matching-block machinery.
struct SequenceMatcher<'s, T> {
    a: &'s [T],
    b: &'s [T],
    /// Positions of each element in `b`, in ascending order, with popular
    /// elements dropped per difflib's autojunk heuristic.
    b2j: HashMap<&'s T, Vec<usize>>,
}

impl<'s, T: Eq + std::hash::Hash> SequenceMatcher<'s, T> {
    fn new(a: &'s [T], b: &'s [T]) -> Self {
        let mut b2j: HashMap<&'s T, Vec<usize>> = HashMap::new();
        for (j, item) in b.iter().enumerate() {
            b2j.entry(item).or_default().push(j);
        }
        // Autojunk: for sequences of >= 200 items, elements occurring in
        // more than 1% of positions cannot seed a match (difflib default).
        if b.len() >= 200 {
            let threshold = b.len() / 100 + 1;
            b2j.retain(|_, positions| positions.len() <= threshold);
        }
        Self { a, b, b2j }
    }

    /// Longest matching block within `a[alo..ahi]` / `b[blo..bhi]`, as
    /// `(besti, bestj, bestsize)` — difflib's `find_longest_match`.
    fn find_longest_match(
        &self,
        alo: usize,
        ahi: usize,
        blo: usize,
        bhi: usize,
    ) -> (usize, usize, usize) {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(positions) = self.b2j.get(&self.a[i]) {
                for &j in positions {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = j
                        .checked_sub(1)
                        .and_then(|prev| j2len.get(&prev).copied())
                        .unwrap_or(0)
                        + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        // Extend the match over equal elements that could not seed it
        // (popular elements dropped from b2j by autojunk).
        while besti > alo && bestj > blo && self.a[besti - 1] == self.b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi
            && bestj + bestsize < bhi
            && self.a[besti + bestsize] == self.b[bestj + bestsize]
        {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    /// Total length of all matching blocks (recursive divide-and-conquer,
    /// like difflib's `get_matching_blocks`).
    fn matching_len(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> usize {
        if alo >= ahi || blo >= bhi {
            return 0;
        }
        let (i, j, size) = self.find_longest_match(alo, ahi, blo, bhi);
        if size == 0 {
            return 0;
        }
        size + self.matching_len(alo, i, blo, j) + self.matching_len(i + size, ahi, j + size, bhi)
    }
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
        // difflib: SequenceMatcher(None, "qabxcd", "abycdf").ratio() == 2*4/12
        assert!((str_ratio("qabxcd", "abycdf") - 4.0 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn autojunk_matches_difflib_semantics() {
        // Ground truth from CPython difflib.SequenceMatcher (autojunk on).
        // Identical uniform sequences still score 1.0: popular elements
        // cannot *seed* a match but the extension loops grow one from the
        // block boundaries.
        let uniform: Vec<String> = vec!["tr".to_string(); 300];
        assert_eq!(seq_ratio(&uniform, &uniform), 1.0);

        // Interrupted uniform sequence: difflib gives exactly 0.5.
        let mut interrupted: Vec<String> = vec!["tr".to_string(); 150];
        interrupted.push("div".to_string());
        interrupted.extend(vec!["tr".to_string(); 149]);
        assert_eq!(seq_ratio(&uniform, &interrupted), 0.5);

        // Where autojunk really bites: a distinctive run displaced across a
        // popular block cannot be re-seeded through the popular elements.
        // difflib: 0.011857707509881422 (vs 0.988 with autojunk=False).
        let mut a: Vec<String> = vec!["x".to_string(); 250];
        a.extend(["a", "b", "c"].map(String::from));
        let mut b: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        b.extend(vec!["x".to_string(); 250]);
        assert!((seq_ratio(&a, &b) - 0.011857707509881422).abs() < 1e-12);
    }

    #[test]
    fn relocation_on_large_table_completes_quickly() {
        // Regression: the pre-index matcher ran an O(a*b) DP per candidate
        // (~19s for 2000 rows in release mode). With the difflib-style
        // index + autojunk this must finish in interactive time even in
        // debug builds.
        let rows: String = (0..1500)
            .map(|i| format!("<tr><td>cell {i}</td></tr>"))
            .collect();
        let html = format!(
            r#"<html><body><table id="data"><tr id="target" class="special"><td>needle</td></tr>{rows}</table></body></html>"#
        );
        let page = Selector::from_html(&html);
        let target = page.css("#target");
        let data = target.first().unwrap().element_data();

        let started = std::time::Instant::now();
        let relocated = page.relocate(&data, DEFAULT_RELOCATION_PERCENTAGE);
        assert!(!relocated.is_empty());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "relocation on a 1500-row table took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn relocate_never_returns_the_html_root_element() {
        // Upstream searches `.//*` from the root element, so <html> itself
        // is never a candidate.
        let page = Selector::from_html("<html><body><p>only child</p></body></html>");
        let data = ElementData {
            tag: "html".to_string(),
            path: vec!["html".to_string()],
            ..Default::default()
        };
        // Even with a threshold of 0 the best match must be a descendant.
        let relocated = page.relocate(&data, 0.0);
        for el in relocated {
            assert_ne!(el.tag(), "html");
        }
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
