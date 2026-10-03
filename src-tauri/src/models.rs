use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    Color,
    Link,
    File,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
            ClipKind::Color => "color",
            ClipKind::Link => "link",
            ClipKind::File => "file",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "image" => ClipKind::Image,
            "color" => ClipKind::Color,
            "link" => ClipKind::Link,
            "file" => ClipKind::File,
            _ => ClipKind::Text,
        }
    }
}

/// A row in the history list. `thumb_path` / `image_path` are stored
/// *relative* to the app data dir; the command layer absolutises them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipListItem {
    pub id: i64,
    pub kind: ClipKind,
    pub title: String,
    pub color: Option<String>,
    pub thumb_path: Option<String>,
    pub image_path: Option<String>,
    pub source_app: Option<String>,
    pub source_bundle: Option<String>,
    /// A free-text note the user attached to this entry.
    ///
    /// Lives on the *list* item rather than only on `ClipDetail` so a row can
    /// show whether it has one before the detail query comes back — otherwise
    /// the list renders a blank and the pane flashes empty. Same category of
    /// data as `pinned`: written only by a user action, never by the capture
    /// path, which is why `NewClipping` has no field for it.
    ///
    /// `None` means "no note". Empty and whitespace-only input is normalised
    /// to `None` on write, so "has a note" is a single test rather than three.
    pub description: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDetail {
    #[serde(flatten)]
    pub item: ClipListItem,
    pub content_text: Option<String>,
    pub content_html: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: Option<i64>,
    pub characters: Option<i64>,
    pub words: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipQuery {
    pub search: Option<String>,
    /// "all" | "text" | "images" | "files" | "links" | "colors"
    pub filter: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    pub hotkey: String,
    pub autostart: bool,
    pub max_items: u32,
    /// Where the user last dropped the panel, in physical pixels.
    ///
    /// `None` until the first drag, and that is the point: while it is unset
    /// the panel keeps centring itself on whichever display holds the cursor,
    /// which is how it behaved before it was draggable.
    ///
    /// `serde(default)` is load-bearing, though not for the reason a reader
    /// might guess — this app has never shipped a schema that lacked the key.
    /// It guards against *format drift*: none of `theme` / `hotkey` /
    /// `autostart` / `max_items` carries a default of its own, so a single
    /// missing key anywhere in the row would fail the whole `Settings`
    /// deserialisation, and `load_settings` answers every failure with
    /// `Default`. One absent optional key would therefore quietly reset the
    /// user's theme, hotkey and history limit along with it. Keeping this one
    /// field tolerant turns that class of accident into a non-event.
    #[serde(default)]
    pub panel_position: Option<PanelPosition>,
}

/// Physical top-left of the panel, as left by the user's last drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelPosition {
    pub x: i32,
    pub y: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            hotkey: "Alt+Super+V".into(),
            autostart: false,
            max_items: 500,
            panel_position: None,
        }
    }
}

/// The frontmost application, used for the `Source` row and the
/// `Paste to <app>` hint.
///
/// Deliberately serialise-only. `AppInfo` never crosses the IPC bridge: it is
/// built by `frontmost::current()`, parked in `state.last_foreground`, and
/// read back entirely inside Rust by `target_app()` / `remember_and_get()`.
/// Everything the webview learns about the source travels through
/// `ClipListItem` (whose app columns are filled straight from SQLite) or
/// through the deliberately narrow `PanelShown { target_app }` payload. There
/// is no `Deserialize` here because nothing ever hands one back to us — and
/// none went missing when it was dropped.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub bundle: Option<String>,
    /// The process id, and the only field that actually lets us hand focus
    /// back to this app.
    ///
    /// `pid` is preferred over `bundle` for two reasons. A bundle identifier
    /// has to be resolved back into a running application every time, and that
    /// lookup can return a different instance than the one the user was in —
    /// or nothing at all for a process macOS has not fully registered yet. And
    /// it is the only identifier that survives `NSWorkspace` returning us
    /// ourselves, which is the case every time the panel is up.
    pub pid: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResult {
    pub target_app: Option<String>,
    /// True when a synthetic Cmd+V was actually posted for the target app.
    pub auto_pasted: bool,
    /// True when auto-paste was refused because the Accessibility permission
    /// is missing; the panel stays up and the UI turns this into a hint.
    pub needs_accessibility: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelShown {
    pub target_app: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A malformed or drifted settings row must not take the whole set down
    /// with it. See the note on `Settings::panel_position` for why a failure
    /// here is destructive rather than merely inconvenient: `load_settings`
    /// answers any error with `Default`, which would reset the theme and
    /// hotkey too.
    #[test]
    fn settings_without_panel_position_still_load() {
        let legacy = r#"{"theme":"dark","hotkey":"Alt+Super+V","autostart":true,"maxItems":250}"#;

        let settings: Settings =
            serde_json::from_str(legacy).expect("a pre-drag settings row must still deserialise");

        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.max_items, 250);
        assert!(settings.autostart);
        assert_eq!(settings.panel_position, None);
    }

    #[test]
    fn panel_position_round_trips() {
        let settings = Settings {
            panel_position: Some(PanelPosition { x: -1280, y: 64 }),
            ..Default::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"panelPosition\""), "got {json}");

        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.panel_position, Some(PanelPosition { x: -1280, y: 64 }));
    }
}
