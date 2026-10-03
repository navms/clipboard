pub mod repo;
pub mod schema;

use std::path::Path;
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::error::Result;

/// Single-writer SQLite handle.
///
/// Captures arrive on the clipboard watcher's native callback thread while
/// commands run on Tauri's worker threads, so all access is funnelled
/// through one mutex-guarded connection. WAL keeps readers from blocking
/// the writer.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        // `journal_mode` returns a row, so it cannot go through execute_batch.
        conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0))?;
        conn.execute_batch("PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
        conn.busy_timeout(Duration::from_millis(5_000))?;

        schema::migrate(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Runs `f` with exclusive access to the connection.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock();
        f(&conn)
    }
}
