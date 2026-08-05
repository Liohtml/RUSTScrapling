//! Shared value types used across the parser and spiders: string wrappers
//! with scraping helpers ([`TextHandler`], [`TextHandlers`]), a read-only
//! attribute map ([`AttributesHandler`]), and the SQLite storage backing
//! adaptive element relocation ([`storage::SqliteStorage`]).

pub mod attributes_handler;
pub mod storage;
pub mod text_handler;
pub mod text_handlers;

pub use attributes_handler::AttributesHandler;
pub use text_handler::TextHandler;
pub use text_handlers::TextHandlers;
