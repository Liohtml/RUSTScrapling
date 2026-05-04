use crate::core::TextHandler;

/// A list of TextHandler values with batch operations.
#[derive(Debug, Clone, Default)]
pub struct TextHandlers {
    items: Vec<TextHandler>,
}

impl TextHandlers {
    pub fn new(items: Vec<TextHandler>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, default: Option<TextHandler>) -> Option<TextHandler> {
        self.items.first().cloned().or(default)
    }

    pub fn getall(&self) -> &[TextHandler] {
        &self.items
    }

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
