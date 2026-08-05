//! SQLite persistence for adaptive element relocation: element snapshots
//! saved by [`Selector::save`](crate::parser::Selector::save) live here,
//! keyed by page URL plus a hashed identifier.

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// SQLite-backed storage for adaptive element relocation.
///
/// One `SqliteStorage` is scoped to a single page URL (lowercased at
/// construction): rows are keyed by `(url, identifier)`, so the same
/// identifier can be reused across different pages without collisions. The
/// connection is guarded by a mutex, making the storage safe to share
/// across threads.
pub struct SqliteStorage {
    conn: Mutex<Connection>,
    url: String,
    db_path: String,
}

impl SqliteStorage {
    /// Open (creating if needed) the SQLite database at `db_path`, scoped
    /// to `url`. The URL is normalized to lowercase; the database uses WAL
    /// journaling and gets its schema created on first use.
    pub fn new(db_path: &str, url: &str) -> Result<Self, StorageError> {
        let conn = Self::open(db_path)?;
        Ok(Self {
            conn: Mutex::new(conn),
            url: url.to_lowercase(),
            db_path: db_path.to_string(),
        })
    }

    /// Open and initialize the SQLite connection at `db_path`. Used both at
    /// construction and to recover after a poisoned mutex.
    fn open(db_path: &str) -> Result<Connection, StorageError> {
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
        Ok(conn)
    }

    /// Lock the connection. If the mutex is poisoned (a previous holder
    /// panicked), recover by reopening a fresh connection — rusqlite does not
    /// guarantee a `Connection` remains usable after a panic mid-operation,
    /// so reopening is safer than reusing the recovered handle.
    fn locked_conn(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poison) => {
                let mut guard = poison.into_inner();
                if let Ok(fresh) = Self::open(&self.db_path) {
                    *guard = fresh;
                }
                guard
            }
        }
    }

    /// Save element data under `identifier` for this storage's URL,
    /// replacing any previous entry (INSERT OR REPLACE upsert).
    pub fn save(
        &self,
        identifier: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<(), StorageError> {
        let hash = Self::get_hash(identifier);
        let json = serde_json::to_string(data)?;
        let conn = self.locked_conn();
        conn.execute(
            "INSERT OR REPLACE INTO storage (url, identifier, element_data) VALUES (?1, ?2, ?3)",
            params![self.url, hash, json],
        )?;
        Ok(())
    }

    /// Retrieve the element data stored under `identifier` for this
    /// storage's URL. Returns `Ok(None)` when nothing was saved yet; real
    /// database errors propagate as `Err`.
    pub fn retrieve(
        &self,
        identifier: &str,
    ) -> Result<Option<HashMap<String, serde_json::Value>>, StorageError> {
        let hash = Self::get_hash(identifier);
        let conn = self.locked_conn();
        let mut stmt =
            conn.prepare("SELECT element_data FROM storage WHERE url = ?1 AND identifier = ?2")?;
        // `.optional()` maps only `QueryReturnedNoRows` to `None`; any other
        // error (locked file, corrupt DB, type mismatch) still propagates
        // instead of being indistinguishable from "not saved yet".
        let result: Option<String> = stmt
            .query_row(params![self.url, hash], |row| row.get(0))
            .optional()?;
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

/// Errors from [`SqliteStorage`] operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The underlying SQLite operation failed (I/O, locking, schema, …).
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Element data could not be serialized to, or deserialized from, its
    /// stored JSON form.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn storage_recovers_after_mutex_poisoning() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("storage.db");
        let storage =
            Arc::new(SqliteStorage::new(db.to_str().unwrap(), "https://example.com").unwrap());

        // Poison the mutex by panicking *while* holding the guard.
        let s = storage.clone();
        let _ = thread::spawn(move || {
            let _guard = s.conn.lock().unwrap();
            panic!("intentional poisoning while holding the guard");
        })
        .join();

        // Confirm the recovery path is actually exercised.
        assert!(
            storage.conn.is_poisoned(),
            "mutex should be poisoned after the thread panicked while holding the guard"
        );

        let mut data = HashMap::new();
        data.insert("k".to_string(), serde_json::Value::String("v".to_string()));
        storage
            .save("after-poison", &data)
            .expect("save must recover from poisoned mutex");
        let got = storage
            .retrieve("after-poison")
            .expect("retrieve must recover from poisoned mutex")
            .expect("the row written after recovery should be present");
        assert_eq!(
            got.get("k").cloned(),
            Some(serde_json::Value::String("v".to_string()))
        );
    }

    #[test]
    fn retrieve_propagates_real_sqlite_errors_instead_of_swallowing_them() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("storage.db");
        let storage = SqliteStorage::new(db.to_str().unwrap(), "https://example.com").unwrap();

        // Drop the table out from under the connection so the next SELECT
        // fails with a real SQLite error (not "no rows"). Before switching to
        // `.optional()`, `retrieve` used `.ok()`, which mapped ANY error —
        // including this one — to `Ok(None)`, indistinguishable from "not
        // saved yet".
        {
            let conn = storage.conn.lock().unwrap();
            conn.execute("DROP TABLE storage", []).unwrap();
        }

        let result = storage.retrieve("whatever");
        assert!(
            result.is_err(),
            "a genuine SQLite error must propagate, not be swallowed as None"
        );
    }
}
