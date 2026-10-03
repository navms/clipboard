mod clipboard;
mod commands;
mod display;
mod error;
mod frontmost;
mod hotkey;
mod media;
mod models;
mod paste;
mod state;
mod store;
mod tray;
mod window;

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::state::AppState;
use crate::store::{repo, Db};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--silent"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<Arc<AppState>>().inner().clone();
                        window::toggle_panel(app, &state);
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                // Accessory: no Dock icon, no Cmd-Tab entry; the panel is
                // reached through the global hotkey or the menu-bar icon.
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);

                // Which Accessory turns out not to be sufficient for the Dock
                // half of that sentence. It governs whether we own the menu bar
                // and appear in Cmd-Tab; the tile in the Dock is a separate,
                // more stubborn thing, governed by whether the *process* is a
                // UI element. The packager gets it right by way of
                // `Info.plist` → `LSUIElement`, and `dev` has no bundle to read
                // that from, so the process is transformed here as well.
                //
                // Delay is the whole trick. tao drops a hide that lands within
                // a second of any show — its workaround for a macOS bug that
                // would otherwise leave duplicate tiles behind — and every
                // value involved is initialised to nothing, so nothing reports
                // the refusal either. Asking from startup means asking from
                // inside that window. See `DOCK_SHOW_TIMEOUT` in
                // tao's `platform_impl/macos/dock.rs`.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));

                    if let Err(err) = handle.set_dock_visibility(false) {
                        eprintln!("[dock] could not hide the Dock tile: {err}");
                    }
                });
            }

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let db = Db::open(&data_dir.join("clipboard.db"))?;
            let settings = db.with(repo::load_settings)?;
            let state = Arc::new(AppState::new(db, data_dir, settings));

            app.manage(state.clone());

            reconcile_assets(&state);

            let hotkey = state.settings.read().hotkey.clone();
            if let Err(err) = app.global_shortcut().register(hotkey.as_str()) {
                eprintln!("[hotkey] could not register {hotkey}: {err}");
            }

            // Second entry point: double-tap Option. Additive to the hotkey above,
            // so a missing Accessibility permission degrades instead of
            // breaking. Registered on the main thread, where AppKit wants it.
            hotkey::register(app.handle(), &state);

            tray::init(app.handle(), state.clone())?;

            // The watcher blocks, so it gets its own thread.
            clipboard::watcher::spawn(app.handle().clone(), state.clone());

            // The panel ships hidden, which leaves "does the webview actually
            // render?" unanswerable from anything but a keystroke. This is the
            // one hook that lets an automated check, or a debugging session,
            // put it on screen without one. No-op unless the var is set.
            if std::env::var_os("CLIPBOARD_SHOW_ON_START").is_some() {
                window::show_panel(app.handle(), &state);
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // The panel is a singleton: closing it just hides it.
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) => {
                // Debounced: transient focus blips while the panel appears
                // should not dismiss it.
                debounce_hide(window.app_handle());
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_clips,
            commands::get_clip_detail,
            commands::pin_clip,
            commands::set_description,
            commands::delete_clip,
            commands::clear_clips,
            commands::paste_clip,
            commands::copy_clip,
            commands::hide_panel,
            commands::begin_panel_drag,
            commands::reset_panel_position,
            commands::open_accessibility_settings,
            commands::get_settings,
            commands::update_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the tauri application")
        .run(|app, event| match event {
            // The Dock icon being clicked with no visible window. macOS sends
            // `reopen` rather than showing anything itself, and nothing answers
            // it unless asked to here — so the click used to do nothing at all.
            // `show_panel` also un-hides the application, which is the other
            // half of what makes this work.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                let state = app.state::<Arc<AppState>>().inner().clone();
                window::show_panel(app, &state);
            }
            _ => {}
        })
}

/// How long the panel gets to regain focus after it loses it.
///
/// Losing focus is how the panel learns it has been dismissed, and it also
/// happens *while the panel is appearing* — AppKit delivers it during show.
/// Acting on the first one would make the panel impossible to open, so it is
/// always given this long to settle.
const BLUR_GRACE: Duration = Duration::from_millis(120);

/// Drops rows any of whose backing files vanished from disk.
///
/// Both columns are checked rather than only the original. A row whose
/// thumbnail alone is gone is exactly as undisplayable as one whose source
/// went — the panel draws the thumbnail — and either kind of orphan is at once
/// unreachable and unrepairable, so the row and whatever assets it still had
/// go together.
fn reconcile_assets(state: &Arc<AppState>) {
    let Ok(rows) = state.db.with(repo::rows_with_assets) else {
        return;
    };

    for (id, image, thumb) in rows {
        let orphaned = [image.as_deref(), thumb.as_deref()]
            .into_iter()
            .flatten()
            .any(|rel| !state.data_dir.join(rel).exists());

        if orphaned {
            drop_row(state, id);
        }
    }
}

fn drop_row(state: &Arc<AppState>, id: i64) {
    if let Ok(assets) = state.db.with(|conn| repo::delete(conn, id)) {
        for rel in assets {
            media::remove(&state.data_dir, &rel);
        }
    }
}

/// Schedules a blur dismissal, restarting the wait if one is already pending.
///
/// The delay is not politeness (see [`BLUR_GRACE`]), but arming it used to
/// mean spawning a thread per event, each sleeping out the full grace period
/// and each deciding on its own afterwards. A few quick summons left a few
/// such threads racing to hide the same window, none of them aware of the
/// others, and no amount of arrival order guaranteed the last one in got the
/// last word.
///
/// One resident timer instead: what it is waiting for simply resets, so the
/// panel is given its grace period from the most recent blur rather than from
/// whichever one happened to be first.
fn debounce_hide(app: &AppHandle) {
    static TIMER: OnceLock<Sender<()>> = OnceLock::new();

    let jobs = TIMER.get_or_init(|| {
        let (jobs, inbox) = mpsc::channel::<()>();
        let handle = app.clone();

        std::thread::Builder::new()
            .name("clipboard-blur".into())
            .spawn(move || {
                while inbox.recv().is_ok() {
                    // Restart the wait for as long as blurs keep arriving:
                    // only the world as it looks after the last one matters.
                    let mut settled = false;
                    while !settled {
                        match inbox.recv_timeout(BLUR_GRACE) {
                            Ok(()) => continue,
                            Err(RecvTimeoutError::Timeout) => settled = true,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }

                    // Asked again rather than assumed: focus can come back
                    // during the grace period, and hiding the panel over the
                    // top of that would fight the user for no reason. Nothing
                    // being answerable counts as still focused, and so still
                    // worth keeping.
                    let refocused = handle
                        .get_webview_window(crate::window::PANEL_LABEL)
                        .and_then(|win| win.is_focused().ok())
                        .unwrap_or(true);
                    if refocused {
                        continue;
                    }

                    let state = handle.state::<Arc<AppState>>().inner().clone();
                    // Routed through `window::hide_panel` rather than
                    // `win.hide()` so a dismissal-by-blur also banks the
                    // position of a drag that just ended.
                    crate::window::hide_panel(&handle, &state);
                }
            })
            .ok();

        jobs
    });

    if jobs.send(()).is_err() {
        eprintln!("[window] blur timer is gone; dismissing immediately");
        let state = app.state::<Arc<AppState>>().inner().clone();
        crate::window::hide_panel(app, &state);
    }
}
