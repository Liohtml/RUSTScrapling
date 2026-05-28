use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

/// SQLite-backed storage for adaptive element relocation.
pub struct SqliteStorage {
    conn: Mutex<Connection>,
    url: String,
}

impl SqliteStorage {
    /// Create storage backed by SQLite file. URL is normalized to lowercase.
    pub fn new(db_path: &str, url: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS storage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                identifier TEXT NOT NULL,
                element_data TEXT NOT NULL,
                UNIQUE(url, identifier)
            )",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            url: url.to_lowercase(),
        })
    }

    /// Save element data. Uses INSERT OR REPLACE for upsert.
    pub fn save(
        &self,
        identifier: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<(), StorageError> {
        let hash = Self::get_hash(identifier);
        let json = serde_json::to_string(data)?;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO storage (url, identifier, element_data) VALUES (?1, ?2, ?3)",
            params![self.url, hash, json],
        )?;
        Ok(())
    }

    /// Retrieve stored element data.
    pub fn retrieve(
        &self,
        identifier: &str,
    ) -> Result<Option<HashMap<String, serde_json::Value>>, StorageError> {
        let hash = Self::get_hash(identifier);
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt =
            conn.prepare("SELECT element_data FROM storage WHERE url = ?1 AND identifier = ?2")?;
        let result: Option<String> = stmt
            .query_row(params![self.url, hash], |row| row.get(0))
            .ok();
        match result {
            Some(json) => {
                let data = serde_json::from_str(&json)?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    fn get_hash(identifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(identifier.as_bytes());
        format!("{:x}_{}", hasher.finalize(), identifier.len())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
