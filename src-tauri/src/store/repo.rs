use rusqlite::{params, Connection, OptionalExtension, Row};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::models::{ClipDetail, ClipKind, ClipListItem, ClipQuery, Settings};

/// Columns shared by every row read.
const ITEM_COLUMNS: &str = "id, kind, content_text, image_path, thumb_path, width, height, \
     source_app, source_bundle, description, pinned, created_at, updated_at";

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A capture that has been classified and hashed, ready to be persisted.
pub struct NewClipping {
    pub kind: ClipKind,
    pub content_text: Option<String>,
    pub content_html: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: Option<i64>,
    pub hash: String,
    pub source_app: Option<String>,
    pub source_bundle: Option<String>,
}

/// What the paste path needs to write a clip back to the pasteboard.
/// (Rich-text flavours are P2; `content_html` stays in the table until then.)
pub struct ClipPayload {
    pub kind: ClipKind,
    pub content_text: Option<String>,
    pub image_path: Option<String>,
}

pub enum UpsertOutcome {
    Inserted(i64),
    Bumped(i64),
}

// --------------------------------------------------------------------------
// Title derivation
// --------------------------------------------------------------------------

fn truncate_chars(value: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in value.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// The single-line label shown in the history list. Derived on read so the
/// table stays free of denormalised display data.
pub fn derive_title(
    kind: ClipKind,
    content: Option<&str>,
    width: Option<i64>,
    height: Option<i64>,
) -> String {
    match kind {
        ClipKind::Image => match (width, height) {
            (Some(w), Some(h)) => format!("Image ({w}×{h})"),
            _ => "Image".to_string(),
        },
        ClipKind::File => file_title(content),
        _ => {
            let raw = content.unwrap_or_default();
            let first_line = raw.lines().next().unwrap_or("").trim();
            let source = if first_line.is_empty() {
                raw.trim()
            } else {
                first_line
            };
            if source.is_empty() {
                "(empty)".to_string()
            } else {
                truncate_chars(source, 80)
            }
        }
    }
}

/// The label for a file clip.
///
/// `content` is the copied paths joined by `\n` (see the watcher), so this must
/// look at the paths individually. Running `Path::file_name()` straight over
/// the joined string, the obvious first draft, reports the *last* path's file
/// name as the title of the whole clip, which is silently wrong the moment a
/// user copies more than one item: a two-file copy titled "budget.xlsx" reads
/// as if only that file had been copied.
///
/// One path therefore gets its name, several get a count.
fn file_title(content: Option<&str>) -> String {
    let paths: Vec<&str> = content
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    match paths.as_slice() {
        [] => "File".to_string(),
        [only] => std::path::Path::new(only)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "File".to_string()),
        many => format!("{} files", many.len()),
    }
}

fn row_to_item(row: &Row<'_>) -> rusqlite::Result<ClipListItem> {
    // `get_ref` borrows the cell straight out of SQLite's row rather than
    // materialising a `String` that is parsed and dropped two lines later.
    // Repeat that per clip per list refresh and it is 500 allocations to make
    // 500 temporary ≤5-byte strings.
    let kind = ClipKind::parse(row.get_ref("kind")?.as_str()?);
    let content_text: Option<String> = row.get("content_text")?;
    let width: Option<i64> = row.get("width")?;
    let height: Option<i64> = row.get("height")?;

    Ok(ClipListItem {
        id: row.get("id")?,
        kind,
        title: derive_title(kind, content_text.as_deref(), width, height),
        color: if kind == ClipKind::Color {
            content_text
        } else {
            None
        },
        thumb_path: row.get("thumb_path")?,
        image_path: row.get("image_path")?,
        source_app: row.get("source_app")?,
        source_bundle: row.get("source_bundle")?,
        description: row.get("description")?,
        pinned: row.get::<_, i64>("pinned")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

// --------------------------------------------------------------------------
// Writes
// --------------------------------------------------------------------------

pub fn upsert(conn: &Connection, clip: &NewClipping) -> Result<UpsertOutcome> {
    let existing: Option<i64> = conn
        .prepare_cached("SELECT id FROM clippings WHERE hash = ?1")?
        .query_row(params![clip.hash], |row| row.get(0))
        .optional()?;

    let now = now_millis();

    if let Some(id) = existing {
        // Re-copying the same content bumps it to the top instead of
        // inserting a duplicate row.
        conn.prepare_cached("UPDATE clippings SET created_at = ?1, updated_at = ?1 WHERE id = ?2")?
            .execute(params![now, id])?;
        return Ok(UpsertOutcome::Bumped(id));
    }

    conn.prepare_cached(
        "INSERT INTO clippings (
            kind, content_text, content_html, image_path, thumb_path,
            width, height, byte_size, hash, source_app, source_bundle,
            pinned, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,0,?12,?12)",
    )?
    .execute(params![
        clip.kind.as_str(),
        clip.content_text,
        clip.content_html,
        clip.image_path,
        clip.thumb_path,
        clip.width,
        clip.height,
        clip.byte_size,
        clip.hash,
        clip.source_app,
        clip.source_bundle,
        now,
    ])?;

    Ok(UpsertOutcome::Inserted(conn.last_insert_rowid()))
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<()> {
    conn.prepare_cached("UPDATE clippings SET pinned = ?1, updated_at = ?2 WHERE id = ?3")?
        .execute(params![pinned as i64, now_millis(), id])?;
    Ok(())
}

/// Attaches a note to a row, or clears it with `None`.
///
/// Blank input collapses to `NULL` here rather than at the call sites, because
/// the list renders a "has a note" affordance: a row storing `""` would show
/// no pencil while still carrying a non-null value, and every future consumer
/// would have to re-derive the same rule. One place decides, so `None` is the
/// only spelling of "empty".
pub fn set_description(conn: &Connection, id: i64, description: Option<&str>) -> Result<()> {
    let normalised = description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    conn.prepare_cached("UPDATE clippings SET description = ?1, updated_at = ?2 WHERE id = ?3")?
        .execute(params![normalised, now_millis(), id])?;
    Ok(())
}

/// Deletes a row and hands back the asset paths that should be unlinked.
pub fn delete(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let assets = asset_paths(conn, "id = ?1", params![id])?;
    conn.prepare_cached("DELETE FROM clippings WHERE id = ?1")?
        .execute(params![id])?;
    Ok(assets)
}

/// Clears history, optionally sparing pinned entries.
pub fn clear(conn: &Connection, keep_pinned: bool) -> Result<Vec<String>> {
    let where_clause = if keep_pinned { "pinned = 0" } else { "1 = 1" };
    let assets = asset_paths(conn, where_clause, [])?;

    let sql = format!("DELETE FROM clippings WHERE {where_clause}");
    conn.prepare_cached(&sql)?.execute([])?;

    Ok(assets)
}

/// Everything ranking below the newest `?1` entries, pinned rows excepted.
///
/// Spelled once so that the `SELECT` gathering a row's assets and the `DELETE`
/// dropping it can never drift apart: if they disagreed, `clear` and `prune`
/// would happily leave orphaned files behind.
const OVERFLOW: &str = "pinned = 0 AND id NOT IN (
        SELECT id FROM clippings WHERE pinned = 0 ORDER BY created_at DESC LIMIT ?1
    )";

/// LRU eviction beyond `max_items`, ignoring pinned rows.
pub fn prune(conn: &Connection, max_items: u32) -> Result<Vec<String>> {
    let assets = asset_paths(conn, OVERFLOW, params![max_items as i64])?;

    let sql = format!("DELETE FROM clippings WHERE {OVERFLOW}");
    conn.prepare_cached(&sql)?
        .execute(params![max_items as i64])?;

    Ok(assets)
}

/// The image and thumb paths for rows matching `where_clause`.
///
/// `where_clause` is caller-supplied but never user-supplied: there are four
/// spellings in this file and nothing outside it can reach the function, so
/// the prepared-statement cache stays bounded.
fn asset_paths<P: rusqlite::Params>(
    conn: &Connection,
    where_clause: &str,
    params: P,
) -> Result<Vec<String>> {
    let sql = format!("SELECT image_path, thumb_path FROM clippings WHERE {where_clause}");
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params, |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (image, thumb) = row?;
        out.extend(image);
        out.extend(thumb);
    }
    Ok(out)
}

// --------------------------------------------------------------------------
// Reads
// --------------------------------------------------------------------------

fn filter_kind(filter: Option<&str>) -> Option<ClipKind> {
    match filter.unwrap_or("all") {
        "text" => Some(ClipKind::Text),
        "images" => Some(ClipKind::Image),
        "files" => Some(ClipKind::File),
        "links" => Some(ClipKind::Link),
        "colors" => Some(ClipKind::Color),
        _ => None,
    }
}

pub fn list(conn: &Connection, query: &ClipQuery) -> Result<Vec<ClipListItem>> {
    use rusqlite::types::Value;

    let mut sql = format!("SELECT {ITEM_COLUMNS} FROM clippings");
    let mut clauses: Vec<String> = Vec::new();
    let mut binds: Vec<Value> = Vec::new();

    if let Some(kind) = filter_kind(query.filter.as_deref()) {
        binds.push(Value::Text(kind.as_str().to_string()));
        clauses.push(format!("kind = ?{}", binds.len()));
    }

    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        binds.push(Value::Text(format!("%{search}%")));
        clauses.push(format!("content_text LIKE ?{} COLLATE NOCASE", binds.len()));
    }

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }

    sql.push_str(" ORDER BY pinned DESC, created_at DESC");

    binds.push(Value::Integer(query.limit.unwrap_or(500) as i64));
    sql.push_str(&format!(" LIMIT ?{}", binds.len()));

    // The statement cache is keyed on SQL text, and this one has a bounded
    // menu of shapes: `filter` resolves to six `kind` values and `search` is
    // present or absent, so twelve variants cover every query this app can
    // issue — each of them compiled once for the lifetime of the process
    // instead of once per panel opening.
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(binds), row_to_item)?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn detail(conn: &Connection, id: i64) -> Result<Option<ClipDetail>> {
    let sql =
        format!("SELECT {ITEM_COLUMNS}, content_html, byte_size FROM clippings WHERE id = ?1");
    let mut stmt = conn.prepare_cached(&sql)?;
    let found = stmt
        .query_row(params![id], |row| {
            let item = row_to_item(row)?;
            let content_text: Option<String> = row.get("content_text")?;
            let characters = content_text.as_ref().map(|t| t.chars().count() as i64);
            let words = content_text
                .as_ref()
                .map(|t| t.split_whitespace().count() as i64);
            Ok(ClipDetail {
                item,
                content_text,
                content_html: row.get("content_html")?,
                width: row.get::<_, Option<i64>>("width")?.map(|v| v as u32),
                height: row.get::<_, Option<i64>>("height")?.map(|v| v as u32),
                byte_size: row.get("byte_size")?,
                characters,
                words,
                // Not a column: `commands::get_clip_detail` overwrites this
                // with a resolved path, or leaves it `None` when the icon
                // could not be had. See `appicon::resolve`.
                source_icon: None,
            })
        })
        .optional()?;
    Ok(found)
}

pub fn payload(conn: &Connection, id: i64) -> Result<Option<ClipPayload>> {
    let mut stmt =
        conn.prepare_cached("SELECT kind, content_text, image_path FROM clippings WHERE id = ?1")?;
    let found = stmt
        .query_row(params![id], |row| {
            Ok(ClipPayload {
                kind: ClipKind::parse(row.get_ref("kind")?.as_str()?),
                content_text: row.get("content_text")?,
                image_path: row.get("image_path")?,
            })
        })
        .optional()?;
    Ok(found)
}

/// A row's id beside whichever asset paths it holds: `(id, original, thumb)`.
///
/// Named rather than spelled out because the bare tuple gives no clue which
/// `Option` holds the original and which the thumbnail, and swapping the two
/// at the call site compiles perfectly while deleting histories instead of
/// rebuilding thumbnails.
pub type AssetRow = (i64, Option<String>, Option<String>);

/// Reads a row's kind + asset paths so startup reconciliation can spot
/// records whose files vanished from disk.
pub fn rows_with_assets(conn: &Connection) -> Result<Vec<AssetRow>> {
    let mut stmt = conn.prepare_cached("SELECT id, image_path, thumb_path FROM clippings")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

// --------------------------------------------------------------------------
// Settings (key/value in the same database; no second store)
// --------------------------------------------------------------------------

pub fn load_settings(conn: &Connection) -> Result<Settings> {
    let raw: Option<String> = conn
        .prepare_cached("SELECT value FROM settings WHERE key = 'app'")?
        .query_row([], |row| row.get(0))
        .optional()?;

    Ok(raw
        .and_then(|json| match serde_json::from_str::<Settings>(&json) {
            Ok(settings) => Some(settings),
            Err(err) => {
                // Falling back to `Default` is deliberate — a settings row
                // nobody can read should not keep the app from starting — but
                // it must never be silent. The fallback does not merely lose
                // one key, it resets every one of them, and without this line
                // the user has no way to learn why their theme and hotkey
                // came back wrong or where the row went.
                eprintln!(
                    "[store] settings row will not parse ({err}); falling back \
                     to defaults, which resets theme, hotkey and panel position. \
                     Row was: {json}"
                );
                None
            }
        })
        .unwrap_or_default())
}

pub fn save_settings(conn: &Connection, settings: &Settings) -> Result<()> {
    // Deliberately no `"{}"` fallback here. Only `panel_position` carries a
    // serde default, so such a row could never be read back — `load_settings`
    // would answer it with `Default`, and the rescue would have quietly wiped
    // the configuration it was protecting. Better to refuse to write than to
    // write something unreadable.
    let json = serde_json::to_string(settings)?;
    conn.prepare_cached(
        "INSERT INTO settings (key, value) VALUES ('app', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )?
    .execute(params![json])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{derive_title, file_title};
    use crate::models::ClipKind;

    #[test]
    fn single_path_keeps_its_file_name() {
        assert_eq!(file_title(Some("/Users/me/docs/report.pdf")), "report.pdf");
    }

    #[test]
    fn several_paths_are_counted_not_misnamed() {
        // The regression this guards: `Path::file_name()` over the joined
        // string returned "two.txt" for this input, i.e. one of the files
        // standing in for all of them.
        assert_eq!(file_title(Some("/a/one.md\n/b/two.txt")), "2 files");
        assert_eq!(file_title(Some("/a\n/b\n/c")), "3 files");
    }

    #[test]
    fn blank_or_missing_content_falls_back() {
        assert_eq!(file_title(None), "File");
        assert_eq!(file_title(Some("")), "File");
        assert_eq!(file_title(Some("  \n \n")), "File");
    }

    #[test]
    fn trailing_and_leading_whitespace_is_ignored() {
        assert_eq!(file_title(Some("  /a/one.md  \n")), "one.md");
    }

    #[test]
    fn file_kind_routes_through_file_title() {
        assert_eq!(
            derive_title(ClipKind::File, Some("/x/y.png"), None, None),
            "y.png"
        );
        // A file clip has no width/height, so the image branch must not be hit.
        assert_eq!(
            derive_title(ClipKind::File, Some("/x/a\n/y/b"), Some(1), Some(1)),
            "2 files"
        );
    }

    // ------------------------------------------------------------------
    // Notes. The first tests in this module to touch a database at all —
    // everything above is a pure function. They need to be: the invariants
    // worth protecting here are about what lands in the database, and no
    // amount of unit-testing `derive_title` would notice a regression in
    // `set_description`. `rusqlite` ships with `bundled`, so an in-memory
    // database is available without pulling in a fixture crate.
    // ------------------------------------------------------------------

    use super::{detail, set_description, upsert, NewClipping, UpsertOutcome};
    use crate::store::schema;
    use rusqlite::Connection;

    /// A schema-migrated in-memory database holding one text entry.
    fn db_with_text_entry() -> (Connection, i64) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        schema::migrate(&conn).expect("migrate");
        let clip = NewClipping {
            kind: ClipKind::Text,
            content_text: Some("capture while unfocused".to_owned()),
            content_html: None,
            image_path: None,
            thumb_path: None,
            width: None,
            height: None,
            byte_size: None,
            hash: "hash-1".to_owned(),
            source_app: Some("Google Chrome".to_owned()),
            source_bundle: None,
        };
        let UpsertOutcome::Inserted(id) = upsert(&conn, &clip).expect("insert") else {
            panic!("expected a fresh row");
        };
        (conn, id)
    }

    fn description_of(conn: &Connection, id: i64) -> Option<String> {
        // `description` reaches `ClipDetail` through the flattened list item,
        // so it is read off `item` rather than off the detail itself.
        detail(conn, id)
            .expect("detail")
            .expect("row")
            .item
            .description
    }

    #[test]
    fn a_new_row_has_no_note() {
        let (conn, id) = db_with_text_entry();
        assert_eq!(description_of(&conn, id), None);
    }

    #[test]
    fn a_note_survives_a_round_trip() {
        let (conn, id) = db_with_text_entry();
        set_description(&conn, id, Some("验收时按这两步走")).expect("set note");
        assert_eq!(
            description_of(&conn, id).as_deref(),
            Some("验收时按这两步走")
        );
    }

    #[test]
    fn a_note_reads_back_through_the_list_query() {
        // `description` is on `ClipListItem`, so it has to come back from
        // `list()` too — that is the query the panel's row list actually uses.
        let (conn, id) = db_with_text_entry();
        set_description(&conn, id, Some("写给未来的自己")).expect("set note");

        let found = super::list(&conn, &crate::models::ClipQuery::default()).expect("list");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].description.as_deref(), Some("写给未来的自己"));
    }

    #[test]
    fn a_blank_note_stores_as_absent() {
        // Whitespace collapsing lives in `set_description` so the list's
        // "has a note" affordance can rely on `None` meaning empty. Without
        // it a row would carry a non-null value while showing no pencil.
        let (conn, id) = db_with_text_entry();
        for blank in ["", "   ", "\n\n", " \t "] {
            set_description(&conn, id, Some(blank)).expect("set note");
            assert_eq!(description_of(&conn, id), None, "input {blank:?}");
        }
    }

    #[test]
    fn a_note_can_be_cleared() {
        let (conn, id) = db_with_text_entry();
        set_description(&conn, id, Some("先写点")).expect("set note");
        set_description(&conn, id, None).expect("clear note");
        assert_eq!(description_of(&conn, id), None);
    }

    #[test]
    fn re_capturing_the_same_content_keeps_the_note() {
        // The invariant most worth pinning down. Copying the same thing twice
        // takes the `Bumped` branch, which by design touches only the two
        // timestamps — a note is user metadata and must outlive a re-capture.
        // If a future change widens that UPDATE, this fails.
        let (conn, id) = db_with_text_entry();
        set_description(&conn, id, Some("别被重拍冲掉")).expect("set note");

        let again = NewClipping {
            kind: ClipKind::Text,
            content_text: Some("capture while unfocused".to_owned()),
            content_html: None,
            image_path: None,
            thumb_path: None,
            width: None,
            height: None,
            byte_size: None,
            hash: "hash-1".to_owned(),
            source_app: Some("Notes".to_owned()),
            source_bundle: None,
        };
        assert!(matches!(
            upsert(&conn, &again).expect("re-capture"),
            UpsertOutcome::Bumped(bumped) if bumped == id
        ));

        assert_eq!(description_of(&conn, id).as_deref(), Some("别被重拍冲掉"));
    }
}
