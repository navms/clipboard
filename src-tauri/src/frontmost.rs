use crate::models::AppInfo;
use crate::state::AppState;

/// The application that currently owns the menu bar / key window.
#[cfg(target_os = "macos")]
pub fn current() -> Option<AppInfo> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;

    Some(AppInfo {
        name: app.localizedName()?.to_string(),
        bundle: app.bundleIdentifier().map(|id| id.to_string()),
        pid: app.processIdentifier(),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn current() -> Option<AppInfo> {
    None
}

/// Whether `pid` names this very process.
///
/// Compared against our own pid rather than a bundle identifier, and the
/// reason is a trap we fell into: `pnpm dev:app` runs
/// `tauri dev --config '{"identifier":"com.hejin.clipboard.dev"}'`, overriding
/// the identifier declared in `tauri.conf.json`. A bundle comparison against
/// the latter therefore never matches in a dev build — we stopped recognising
/// ourselves, recorded ourselves as the paste target, and then tried to hand
/// focus to ourselves. The pid does not care what the build was configured
/// with.
pub fn is_self(pid: i32) -> bool {
    pid > 0 && pid == std::process::id() as i32
}

/// Frontmost pid, ignoring this process.
#[cfg(target_os = "macos")]
pub fn frontmost_pid() -> Option<i32> {
    current().map(|a| a.pid).filter(|pid| !is_self(*pid))
}

/// Returns the app a capture should be attributed to.
///
/// When the frontmost app is us (the panel is open), fall back to the last
/// non-self app we saw; that is the app the user was actually working in.
pub fn remember_and_get(state: &AppState) -> Option<AppInfo> {
    if let Some(app) = current() {
        if !is_self(app.pid) {
            *state.last_foreground.write() = Some(app.clone());
            return Some(app);
        }
    }
    state.last_foreground.read().clone()
}
