use rusqlite::Connection;

use crate::error::Result;

pub const SCHEMA_VERSION: i64 = 1;

/// Creates the current schema, once per database.
///
/// Single-step by construction: there is nothing here that reshapes an older
/// database, because no older database exists. `user_version` only ever
/// advances from a run of this build, so the table below simply *is* the
/// schema rather than the latest link in a chain of migrations.
///
/// The one consequence worth knowing: `CREATE TABLE IF NOT EXISTS` declines to
/// touch a table it finds already there, so changing any column needs the
/// `.db` file deleted — along with its `-wal` and `-shm` sidecars, which the
/// WAL mode enabled in `store/mod.rs` always leaves beside it.
///
/// `SCHEMA_VERSION` is deliberately still `1` even though `description` was
/// added after the first release of this file. Bumping it would make every
/// database that predates the column *skip* the `CREATE TABLE` and keep the
/// old shape, which is the opposite of the intended outcome; the only correct
/// response to a changed column here is to delete the file. The gate above
/// exists to avoid re-running the batch on every launch, not to sequence
/// migrations — there are none.
pub fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS clippings (
          id            INTEGER PRIMARY KEY AUTOINCREMENT,
          kind          TEXT    NOT NULL,   -- text | image | color | link | file
          content_text  TEXT,               -- text / url / #hex / newline-joined paths
          content_html  TEXT,               -- P2 rich text
          image_path    TEXT,               -- original, relative to app data dir
          thumb_path    TEXT,               -- thumbnail, relative to app data dir
          width         INTEGER,
          height        INTEGER,
          byte_size     INTEGER,
          hash          TEXT    NOT NULL,   -- dedupe fingerprint
          source_app    TEXT,
          source_bundle TEXT,
          description   TEXT,               -- user-authored note; see models.rs
          pinned        INTEGER NOT NULL DEFAULT 0,
          created_at    INTEGER NOT NULL,
          updated_at    INTEGER NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_clip_hash
          ON clippings(hash);
        CREATE INDEX IF NOT EXISTS idx_clip_created
          ON clippings(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_clip_kind_created
          ON clippings(kind, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_clip_pinned_created
          ON clippings(pinned DESC, created_at DESC);

        CREATE TABLE IF NOT EXISTS settings (
          key   TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        "#,
    )?;
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;

    Ok(())
}
