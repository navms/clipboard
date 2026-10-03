use std::path::Path;
use std::sync::Arc;

use clipboard_rs::common::{RustImage, RustImageData};
use clipboard_rs::{Clipboard, ClipboardContext};
use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::{AppError, Result};
use crate::models::{ClipDetail, ClipKind, ClipListItem, ClipQuery, PasteResult, Settings};
use crate::state::AppState;
use crate::store::repo;
use crate::window;

fn absolutise(base: &Path, rel: &str) -> String {
    base.join(rel).to_string_lossy().into_owned()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub theme: Option<String>,
    pub hotkey: Option<String>,
    pub autostart: Option<bool>,
    pub max_items: Option<u32>,
}

// --------------------------------------------------------------------------
// Reads
// --------------------------------------------------------------------------

#[tauri::command]
pub fn list_clips(
    state: State<'_, Arc<AppState>>,
    query: Option<ClipQuery>,
) -> Result<Vec<ClipListItem>> {
    let query = query.unwrap_or_default();
    let mut items = state.db.with(|conn| repo::list(conn, &query))?;

    for item in &mut items {
        item.image_path = item
            .image_path
            .as_deref()
            .map(|p| absolutise(&state.data_dir, p));
        item.thumb_path = item
            .thumb_path
            .as_deref()
            .map(|p| absolutise(&state.data_dir, p));
    }

    Ok(items)
}

#[tauri::command]
pub fn get_clip_detail(state: State<'_, Arc<AppState>>, id: i64) -> Result<Option<ClipDetail>> {
    let detail = state.db.with(|conn| repo::detail(conn, id))?;
    Ok(detail.map(|mut d| {
        d.item.image_path = d
            .item
            .image_path
            .as_deref()
            .map(|p| absolutise(&state.data_dir, p));
        d.item.thumb_path = d
            .item
            .thumb_path
            .as_deref()
            .map(|p| absolutise(&state.data_dir, p));
        // Resolved here rather than in the row read: it is a lookup through
        // AppKit plus a write, neither of which belongs in a query, and only
        // the detail pane has a use for the answer. The list would pay for
        // every row on every refresh to render a field it does not show.
        d.source_icon = crate::appicon::resolve(&state.data_dir, d.item.source_bundle.as_deref());
        d
    }))
}

// --------------------------------------------------------------------------
// Mutations
// --------------------------------------------------------------------------

#[tauri::command]
pub fn pin_clip(state: State<'_, Arc<AppState>>, id: i64, pinned: bool) -> Result<()> {
    state.db.with(|conn| repo::set_pinned(conn, id, pinned))
}

/// Attaches or clears the note on one entry.
///
/// `None` and `""` both mean "no note"; the normalisation lives in
/// `repo::set_description` so the stored value has a single empty spelling.
#[tauri::command]
pub fn set_description(
    state: State<'_, Arc<AppState>>,
    id: i64,
    description: Option<String>,
) -> Result<()> {
    state
        .db
        .with(|conn| repo::set_description(conn, id, description.as_deref()))
}

#[tauri::command]
pub fn delete_clip(state: State<'_, Arc<AppState>>, id: i64) -> Result<()> {
    let assets = state.db.with(|conn| repo::delete(conn, id))?;
    for rel in assets {
        crate::media::remove(&state.data_dir, &rel);
    }
    Ok(())
}

#[tauri::command]
pub fn clear_clips(state: State<'_, Arc<AppState>>, keep_pinned: bool) -> Result<()> {
    let assets = state.db.with(|conn| repo::clear(conn, keep_pinned))?;
    for rel in assets {
        crate::media::remove(&state.data_dir, &rel);
    }
    Ok(())
}

/// Writes to the pasteboard with the watcher's echo suppressed, and disarms
/// the suppression again if the write did not actually happen.
///
/// The flag means "the next capture is ours, ignore it". Nothing of ours comes
/// back through the watcher when the write fails, so leaving it armed spends
/// it on the user's next real Cmd+C instead — one perfectly ordinary copy that
/// simply never arrives in the history, with nothing logged anywhere. `?`
/// straight after `suppress_once()` is how that used to read, and the failure
/// is ordinary too: an image whose file was cleaned up out from under us.
fn write_suppressed(app: &AppHandle, state: &AppState, payload: repo::ClipPayload) -> Result<()> {
    state.suppress_once();
    // `inspect_err` rather than `map_err`: the error itself is passed straight
    // through, untouched, and this is only about what happens on the way past.
    write_pasteboard(app, state, payload).inspect_err(|_| {
        state.take_suppress();
    })
}

/// Copies the clip back to the system pasteboard, then hands the keystroke
/// off so the target app actually *receives* it.
///
/// The order is deliberate. Auto-paste needs the Accessibility permission, so
/// that is checked before anything is torn down: without it we leave the
/// panel up and report `needs_accessibility`, which the UI turns into a hint.
/// Hiding the panel and then quietly doing nothing would be the worst of both
/// worlds: the content is on the pasteboard either way, so the user can
/// always fall back to Cmd+V themselves.
#[tauri::command]
pub fn paste_clip(app: AppHandle, state: State<'_, Arc<AppState>>, id: i64) -> Result<PasteResult> {
    let payload = state
        .db
        .with(|conn| repo::payload(conn, id))?
        .ok_or(AppError::NotFound(id))?;

    write_suppressed(&app, &state, payload)?;

    // Snapshot before hiding: `last_foreground` is what points at the app the
    // user was working in, and its pid is what lets us refocus it.
    let target = state.target_app();
    let target_app = target.as_ref().map(|a| a.name.clone());

    if !crate::paste::is_trusted() {
        return Ok(PasteResult {
            target_app,
            auto_pasted: false,
            needs_accessibility: true,
        });
    }

    window::hide_panel(&app, state.inner());
    crate::paste::deliver(target.and_then(|a| (a.pid > 0).then_some(a.pid)));

    Ok(PasteResult {
        target_app,
        auto_pasted: true,
        needs_accessibility: false,
    })
}

/// Puts the clip on the pasteboard and dismisses the panel, without firing
/// anything at the target app.
///
/// Kept separate from `paste_clip` on purpose: "Copy" means *the user* will
/// paste later, so synthesising Cmd+V here would type into whichever app happens
/// to have focus.
#[tauri::command]
pub fn copy_clip(app: AppHandle, state: State<'_, Arc<AppState>>, id: i64) -> Result<()> {
    let payload = state
        .db
        .with(|conn| repo::payload(conn, id))?
        .ok_or(AppError::NotFound(id))?;

    write_suppressed(&app, &state, payload)?;
    window::hide_panel(&app, state.inner());

    Ok(())
}

fn write_pasteboard(app: &AppHandle, state: &AppState, payload: repo::ClipPayload) -> Result<()> {
    match payload.kind {
        ClipKind::Image => {
            let rel = payload
                .image_path
                .ok_or_else(|| AppError::Other("clip has no image file".into()))?;
            let bytes = std::fs::read(state.data_dir.join(rel))?;
            let image = RustImageData::from_bytes(&bytes)
                .map_err(|e| AppError::Clipboard(e.to_string()))?;
            let ctx = ClipboardContext::new().map_err(|e| AppError::Clipboard(e.to_string()))?;
            ctx.set_image(image)
                .map_err(|e| AppError::Clipboard(e.to_string()))?;
        }
        ClipKind::File => {
            let files: Vec<String> = payload
                .content_text
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.to_string())
                .collect();
            let ctx = ClipboardContext::new().map_err(|e| AppError::Clipboard(e.to_string()))?;
            ctx.set_files(files)
                .map_err(|e| AppError::Clipboard(e.to_string()))?;
        }
        _ => {
            let text = payload.content_text.unwrap_or_default();
            app.clipboard()
                .write_text(text)
                .map_err(|e| AppError::Clipboard(e.to_string()))?;
        }
    }

    Ok(())
}

/// Opens System Settings on the Accessibility pane. Lives on the Rust side
/// because the deep link is a macOS implementation detail.
#[tauri::command]
pub fn open_accessibility_settings() {
    crate::paste::open_accessibility_settings();
}

#[tauri::command]
pub fn hide_panel(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    window::hide_panel(&app, state.inner());
    Ok(())
}

/// Starts a window move from a `mousedown` on the panel's chrome.
///
/// Rust-side rather than Tauri's `data-tauri-drag-region`, for two reasons.
/// The attribute also binds double-click to `internal_toggle_maximize` (see
/// `tauri/src/window/scripts/drag.js`), which is the wrong gesture for a
/// fixed-size launcher panel: here double-click puts it back in the middle.
/// And owning the command keeps `core:window:allow-start-dragging` out of the
/// capability file: the webview never gets a general window-moving primitive.
///
/// That second claim has been checked rather than assumed, which is worth
/// recording because it is only true by a margin. `capabilities/default.json`
/// declares `core:default`, which does include `core:window:default` — and
/// that set holds 28 permissions, `allow-internal-toggle-maximize` among them,
/// but *not* `allow-start-dragging`. The webview genuinely cannot move the
/// window. See `gen/schemas/acl-manifests.json`, which is the build's own
/// expansion of those sets; the strings it expands do not ship in the crate.
#[tauri::command]
pub fn begin_panel_drag(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    window::begin_drag(&app, state.inner());
    Ok(())
}

/// Forgets a dragged position and re-centres the panel on the cursor's display.
#[tauri::command]
pub fn reset_panel_position(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    window::reset_panel_position(&app, state.inner());
    Ok(())
}

// --------------------------------------------------------------------------
// Settings
// --------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<Settings> {
    Ok(state.settings.read().clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    patch: SettingsPatch,
) -> Result<Settings> {
    // Read-modify-write under compare-and-swap rather than "snapshot, act,
    // overwrite". Those three steps used to be three separate critical
    // sections: a second caller completing in between would have its own
    // settings silently un-done by us publishing a value computed from a
    // snapshot that no longer reflected anything. Two artifacts at once is not
    // likely; the failure is a setting that quietly refuses to stick, which is
    // worse than unlikely — it is undiagnosable.
    //
    // The write lock is held only across one SQLite insert. Everything that
    // can take real time — moving a launch agent about, re-registering the
    // global shortcut — stays outside it, and outside the retry loop too, so
    // each side effect still runs exactly once.
    //
    // Order note: this takes `settings` before `db`. Nothing anywhere does the
    // reverse while holding the database, `AppState::persist_settings` included,
    // so there is no cycle waiting to be found.
    let (next, previous_hotkey) = loop {
        let current = state.settings.read().clone();
        let previous_hotkey = current.hotkey.clone();
        let mut next = current.clone();

        if let Some(theme) = &patch.theme {
            next.theme = theme.clone();
        }
        if let Some(hotkey) = patch.hotkey.as_deref().filter(|h| !h.trim().is_empty()) {
            next.hotkey = hotkey.to_string();
        }
        if let Some(autostart) = patch.autostart {
            next.autostart = autostart;
        }
        if let Some(max_items) = patch.max_items {
            next.max_items = max_items.max(10);
        }

        if next == current {
            // Every field already matched, including for an empty patch. Worth
            // short-circuiting on its own merits: the alternative pokes
            // launchd about a login item that is already in the requested
            // state, once per no-op save.
            break (next, previous_hotkey);
        }

        let mut guard = state.settings.write();
        if *guard != current {
            // Someone got here first. Fold our patch in again on top of
            // whatever they left rather than clobbering it.
            continue;
        }

        // Store before publish, never the other way round: a value we could
        // not persist must not be the one the running app goes on to serve.
        state.db.with(|conn| repo::save_settings(conn, &next))?;
        *guard = next.clone();
        break (next, previous_hotkey);
    };

    if patch.autostart.is_some() {
        apply_autostart(&app, next.autostart);
    }
    if next.hotkey != previous_hotkey {
        re_register_hotkey(&app, &previous_hotkey, &next.hotkey);
    }

    Ok(next)
}

fn apply_autostart(app: &AppHandle, enabled: bool) {
    // The plugin's accessor is `autolaunch()` (renamed to `autostart` in v3).
    let manager = app.autolaunch();

    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };

    if let Err(err) = result {
        eprintln!("[settings] autostart toggle failed: {err}");
    }
}

fn re_register_hotkey(app: &AppHandle, previous: &str, next: &str) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister(previous);
    if let Err(err) = shortcuts.register(next) {
        eprintln!("[settings] could not register {next}: {err}");
    }
}
