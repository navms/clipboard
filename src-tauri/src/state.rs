use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;

use crate::models::{AppInfo, PanelPosition, Settings};
use crate::store::{repo, Db};

/// Process-wide application state, shared between the watcher thread, the
/// tray/hotkey callbacks and the command handlers.
pub struct AppState {
    pub db: Db,
    pub data_dir: PathBuf,
    pub settings: RwLock<Settings>,
    /// Set immediately before we write to the pasteboard ourselves so the
    /// watcher can ignore its own echo.
    suppress_next: AtomicBool,
    /// Raised when a drag begins, consumed when the resulting position is
    /// written. Without it every hide would persist the auto-centred spot and
    /// quietly turn "follow the cursor" into "stick to one display".
    panel_dragged: AtomicBool,
    /// Most recent frontmost app that was *not* us, used as the paste
    /// target and as the `Source` for captures made while we were focused.
    pub last_foreground: RwLock<Option<AppInfo>>,
}

impl AppState {
    pub fn new(db: Db, data_dir: PathBuf, settings: Settings) -> Self {
        Self {
            db,
            data_dir,
            settings: RwLock::new(settings),
            suppress_next: AtomicBool::new(false),
            panel_dragged: AtomicBool::new(false),
            last_foreground: RwLock::new(None),
        }
    }

    pub fn suppress_once(&self) {
        self.suppress_next.store(true, Ordering::SeqCst);
    }

    /// Consumes the "we just wrote to the pasteboard" flag. Returns `true`
    /// when the change the watcher is looking at was caused by us.
    pub fn take_suppress(&self) -> bool {
        self.suppress_next.swap(false, Ordering::SeqCst)
    }

    pub fn target_app_name(&self) -> Option<String> {
        self.last_foreground.read().as_ref().map(|a| a.name.clone())
    }

    /// The full record: auto-paste needs the bundle id, not just the label.
    pub fn target_app(&self) -> Option<AppInfo> {
        self.last_foreground.read().clone()
    }

    pub fn mark_panel_dragged(&self) {
        self.panel_dragged.store(true, Ordering::SeqCst);
    }

    /// Consumes the "a drag happened" flag, so exactly one hide can turn it
    /// into a persisted position.
    pub fn take_panel_dragged(&self) -> bool {
        self.panel_dragged.swap(false, Ordering::SeqCst)
    }

    /// Records where the user left the panel and writes it through. Called
    /// only for positions the user chose; see `take_panel_dragged`.
    pub fn remember_panel_position(&self, position: PanelPosition) {
        if self.settings.read().panel_position == Some(position) {
            return;
        }
        self.settings.write().panel_position = Some(position);
        self.persist_settings();
    }

    /// Drops the remembered spot so the panel goes back to centring itself.
    pub fn forget_panel_position(&self) {
        if self.settings.read().panel_position.is_none() {
            return;
        }
        self.settings.write().panel_position = None;
        self.persist_settings();
    }

    /// Best-effort write-through; a failed save costs the position, not the
    /// session.
    fn persist_settings(&self) {
        let snapshot = self.settings.read().clone();
        if let Err(err) = self.db.with(|conn| repo::save_settings(conn, &snapshot)) {
            eprintln!("[window] could not persist panel position: {err}");
        }
    }
}
