use std::sync::Arc;

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewWindow};

use crate::display;
use crate::models::{PanelPosition, PanelShown};
use crate::state::AppState;

pub const PANEL_LABEL: &str = "main";
pub const PANEL_SHOWN: &str = "panel://shown";

fn panel(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(PANEL_LABEL)
}

/// Pulls the application back out of AppKit's hidden state and makes it active.
///
/// The whole panel hangs off this one call. `NSApp` goes hidden whenever
/// anything sends it `hide:` — our own `-hide_panel` used to, and a stray
/// Cmd+H from outside still can — and a hidden *application* swallows every
/// `makeKeyAndOrderFront:` aimed at its windows. Nothing errors, nothing logs:
/// the window simply stays away, and every summon walks into the same dead
/// end over and over.
///
/// Must run on the main thread; that is the only requirement AppKit places on
/// it, and every caller here already satisfies it.
#[cfg(target_os = "macos")]
fn activate() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[window] off the main thread, skipping AppKit activation");
        return;
    };

    let ns_app = NSApplication::sharedApplication(mtm);
    // Unhide first: `activateIgnoringOtherApps:` alone does not always drag a
    // hidden application back onto screen, only reason it into doing so.
    ns_app.unhide(None);

    // Apple would rather we said `-activate`, but that variant is
    // *cooperative* — the system is allowed to decline it. This panel exists
    // precisely to steal the foreground from whatever the user was typing
    // into, which is the case cooperative activation gives up on first, so the
    // forcing variant stays. Same call `paste.rs` makes deliberately.
    #[allow(deprecated)]
    ns_app.activateIgnoringOtherApps(true);
}

pub fn toggle_panel(app: &AppHandle, state: &Arc<AppState>) {
    let visible = panel(app)
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false);

    if visible {
        hide_panel(app, state);
    } else {
        show_panel(app, state);
    }
}

pub fn show_panel(app: &AppHandle, state: &Arc<AppState>) {
    let Some(win) = panel(app) else {
        return;
    };

    // Capture the paste target *before* we take focus — all of it, not just
    // the label. `activate()` below is what takes the front away, and
    // auto-paste later asks `target_app()` for the pid it has to hand that
    // front back to; keeping only `name` left nothing to restore from, so the
    // paste aimed at whatever the clipboard watcher happened to have last
    // attributed a copy to — or at nothing, when nothing had.
    //
    // This is the step a non-activating panel gets to skip entirely: it never
    // steals the front, so it never has to give it back.
    let target = crate::frontmost::current().filter(|a| !crate::frontmost::is_self(a.pid));

    if let Some(app) = &target {
        *state.last_foreground.write() = Some(app.clone());
    }

    let target_name = target
        .as_ref()
        .map(|a| a.name.clone())
        .or_else(|| state.target_app_name());

    place_panel(&win, state);

    // Order matters, and not in the obvious direction: the application has to
    // be un-hidden *before* the window is shown, because AppKit drops calls
    // aimed at the windows of a hidden application on the floor.
    #[cfg(target_os = "macos")]
    activate();

    let _ = win.show();
    // Focus is required for the webview to receive key presses. tao guards
    // this behind an `isVisible` check, which is one more reason the order
    // above matters: it no-ops until `show` has actually landed.
    let _ = win.set_focus();

    // Emitted on the next turn of the run loop rather than inline. `show()`
    // only *asks* AppKit to order the window in; the webview's geometry is
    // not necessarily restored by the time this function returns, and the
    // frontend's response to this event is to re-measure the list viewport.
    // Measuring a window that is still waking reads zero, and a zero cached
    // there was the blank rail. One turn costs nothing — the panel is not
    // interactive until the next frame anyway — and buys a webview whose
    // layout has caught up with the event.
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = handle.emit(
            PANEL_SHOWN,
            PanelShown {
                target_app: target_name,
            },
        );
    });
}

/// Puts the panel away, banking its position if the user chose one.
///
/// Only a spot the user picked is worth keeping. Persisting on every hide
/// would write back the auto-centred position, and the panel would stop
/// following the cursor after its very first show.
///
/// What this deliberately does *not* do is hide the application itself. `NSApp`
/// being hidden is precisely the state [`activate`] exists to undo, and it was
/// this very function putting it there — so every summon after the first
/// dismissal met a door nobody had the key for. It bought nothing either: the
/// Mission Control cleanliness it was written for arrives free with
/// `ActivationPolicy::Accessory`.
pub fn hide_panel(_app: &AppHandle, state: &Arc<AppState>) {
    if let Some(win) = panel(_app) {
        if state.take_panel_dragged() {
            if let Some(pos) = bank_position(&win) {
                state.remember_panel_position(pos);
            }
        }

        let _ = win.hide();
    }
}

/// Hands the window to AppKit's own drag loop.
///
/// Called from `mousedown` on the top bar. The loop holds the window key for
/// as long as the mouse is down, which is what keeps the panel from blurring
/// and hiding the moment the pointer crosses its edge. The `panel_dragged`
/// flag is what later separates "the user put it here" from "we centred it".
pub fn begin_drag(app: &AppHandle, state: &Arc<AppState>) {
    let Some(win) = panel(app) else {
        return;
    };

    // Marked before the call, not after: AppKit's drag loop does not return
    // until the mouse comes up, and the hide that ends the gesture can land
    // while we are still inside it.
    state.mark_panel_dragged();

    if let Err(err) = win.start_dragging() {
        eprintln!("[window] could not start the move: {err}");
    }
}

/// Drops the remembered spot and re-centres the panel on the cursor's display.
///
/// A drag with no way back is a trap: the panel is summoned by a key, so
/// there is no title bar to drag it home with once it has been parked
/// somewhere awkward.
pub fn reset_panel_position(app: &AppHandle, state: &Arc<AppState>) {
    state.forget_panel_position();

    if let Some(win) = panel(app) {
        position_on_cursor_monitor(&win);
    }
}

/// Puts the panel where the user last left it, falling back to centring when
/// there is nothing remembered, or when the remembered spot no longer lands
/// on an attached display (undocked laptop, unplugged monitor, changed
/// resolution).
fn place_panel(win: &WebviewWindow, state: &Arc<AppState>) {
    // Copied out on its own line for a reason. Written inline as
    // `if let Some(pos) = state.settings.read().panel_position { .. }`, the
    // read guard lives until the end of the *whole* block, and the
    // `forget_panel_position()` below takes the write lock. `parking_lot`'s
    // RwLock is not reentrant, so that thread wedges: the window never
    // appears, nothing is persisted, and the process stays up looking fine.
    let remembered = state.settings.read().panel_position;

    if let Some(pos) = remembered {
        if panel_rect(win, pos).is_some_and(display::is_reachable) {
            // Stored in display space, so this is a plain move: no backing
            // scale to undo, and the same spot comes back no matter which
            // display the panel is sitting on when it is summoned.
            let _ = win.set_position(LogicalPosition::new(pos.x as f64, pos.y as f64));
            return;
        }

        // Stale: forget it, so the next show does not repeat the sweep.
        state.forget_panel_position();
    }

    position_on_cursor_monitor(win);
}

/// The rectangle a remembered top-left would put the panel in.
fn panel_rect(win: &WebviewWindow, pos: PanelPosition) -> Option<display::Rect> {
    let (w, h) = panel_size(win)?;
    Some(display::Rect::new(pos.x as f64, pos.y as f64, w, h))
}

/// The panel's size in points.
fn panel_size(win: &WebviewWindow) -> Option<(f64, f64)> {
    let scale = backing_scale(win)?;
    let size = win.outer_size().ok()?;
    Some((size.width as f64 / scale, size.height as f64 / scale))
}

/// The window's top-left, in the display space everything else is measured in.
fn bank_position(win: &WebviewWindow) -> Option<PanelPosition> {
    let scale = backing_scale(win)?;
    let pos = win.outer_position().ok()?;

    Some(PanelPosition {
        x: (pos.x as f64 / scale).round() as i32,
        y: (pos.y as f64 / scale).round() as i32,
    })
}

/// The window's backing scale factor, as a usable divisor.
///
/// Everything tao hands back is display space multiplied by this factor;
/// `outer_position` and `outer_size` both push the AppKit rectangle through
/// `to_physical`. Dividing by it puts the value back into the points that
/// `display` works in, which also makes it independent of *which* display the
/// panel is on: a spot banked on a 1x screen used to come back halved the next
/// time the panel opened on a 2x one.
fn backing_scale(win: &WebviewWindow) -> Option<f64> {
    let scale = win.scale_factor().ok()?;
    (scale > 0.0).then_some(scale)
}

/// Centres the panel horizontally on whichever display holds the cursor,
/// sitting it slightly above the vertical middle.
fn position_on_cursor_monitor(win: &WebviewWindow) {
    let Some(screen) = display::under_cursor() else {
        return;
    };
    let Some((w, h)) = panel_size(win) else {
        return;
    };

    let (x, y) = display::placement_on(screen, w, h);
    let _ = win.set_position(LogicalPosition::new(x, y));
}
