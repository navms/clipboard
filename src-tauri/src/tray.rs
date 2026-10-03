use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::AppHandle;

use crate::state::AppState;

/// The menu-bar glyph.
///
/// A *template* image: macOS reads only the alpha channel and tints the shape
/// itself, so it stays legible on light and dark menu bars (and inverts under
/// the open-menu highlight). Handing over the full-colour app icon instead
/// would park a blue rounded square in the menu bar, which is exactly what
/// every other status item avoids.
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray.png");

/// Menu-bar entry point, the only way to reach Quit now that the app runs
/// as an accessory (no Dock icon).
///
/// Deliberately minimal: "Show", "Reset Panel Position" and "Quit".
/// "Reset Panel Position" earns its place because the panel can now be dragged
/// somewhere awkward and is summoned by a keystroke; without this there is no
/// obvious way to bring it home. Destructive or per-entry commands (Clear
/// History) still belong to the in-app Cmd+K menu, where they run against the
/// same `clear_clips` command the tray used to call.
pub fn init(app: &AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Clipboard History", true, None::<&str>)?;
    let recenter = MenuItem::with_id(app, "recenter", "Reset Panel Position", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(app, &[&show, &recenter, &quit])?;

    let tray_state = state.clone();
    let builder = TrayIconBuilder::with_id("clipboard-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => crate::window::show_panel(app, &tray_state),
            "recenter" => crate::window::reset_panel_position(app, &tray_state),
            "quit" => app.exit(0),
            _ => {}
        })
        .icon(tauri::image::Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(true);

    builder.build(app)?;
    Ok(())
}
