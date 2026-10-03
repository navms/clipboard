# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.1.0] — 2026-10-03

First public release.

### Added

**Clipboard capture**

- Background watcher on the system pasteboard, with a dedicated persistence
  worker so rapid `Cmd+C` bursts no longer drop intermediate captures.
- Automatic classification into five kinds: text, image, color, link, and file.
- Per-capture source attribution — the application the copy came from, resolved
  by `NSWorkspace` at the instant of capture.
- BLAKE3 content fingerprints for deduplication.

**History and search**

- SQLite persistence (bundled `rusqlite`, WAL mode) — history survives restarts.
- Grouped by date: Today / Yesterday / Earlier.
- Fuzzy search via Fuse.js, plus a filter per content type.
- Virtualized list rendering, so a long history stays smooth.
- Pinned entries kept at the top; single-entry delete; clear-all that can
  preserve pinned items.
- Image thumbnails on disk, reconciled on startup.

**The panel**

- A floating, always-on-top, borderless panel (760×480) summoned by a global
  hotkey (`⌥⌘V` by default) or by double-tapping the Option key.
- Automatically hides on focus loss; re-centers on whichever display holds the
  cursor, and remembers a dragged position.
- Full keyboard navigation: `↑`/`↓` to move, `Return` to paste, `Esc` to close,
  `⌘K` for the actions menu.
- Detail pane with per-type preview and an Information section (source app,
  content type, size, timestamps).
- Light, dark, and system themes.

**Paste back**

- Pastes directly into the application you were last using, resolving the
  target by `pid` and restoring it with cooperative activation.
- Falls back to clipboard-only copy when auto-paste is not possible.
- Option to copy to the pasteboard without sending a keystroke.

**System integration**

- Menu-bar tray icon (Show, Reset Panel Position, Quit).
- No Dock icon — `LSUIElement` in the packaged build, `set_dock_visibility`
  after a 1.5 s grace period in development.
- Launch at login.
- The app reopens on Dock activation.

### Notes

- Apple Silicon (arm64) only. Intel Macs are not supported.
- Release builds are code-signed but not notarized, so first launch requires
  right-click → Open, or clearing the quarantine attribute.
- Double-tap-Option and auto-paste require the Accessibility permission, which
  is bound to the app's designated requirement — see
  [SECURITY.md](SECURITY.md#security-relevant-design-notes).

[Unreleased]: https://github.com/navms/clipboard/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/navms/clipboard/releases/tag/v0.1.0
