//! HTML parsing and data extraction: CSS selection with `::text`/`::attr()`
//! pseudo-elements, DOM navigation, selector generation, and adaptive
//! element relocation.

pub mod adaptive;
pub mod selector;
pub mod selector_generation;
pub mod selectors;
pub mod translator;

pub use adaptive::{ElementData, DEFAULT_RELOCATION_PERCENTAGE};
pub use selector::Selector;
pub use selectors::Selectors;
