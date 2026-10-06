# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

**Reopening the panel no longer restores the previous selection.** The panel
is a single resident window that is only ever hidden, never torn down, so the
last selected row — and the rail's scroll offset — survived into the next
opening. `panel://shown` reset the query, the type filter and the view but not
the selection, and `refresh()` only reselects when the old row has fallen out
of the list, which history rows never do. Every summon now clears the
per-session state through a single store action and lands on the newest entry
with the list scrolled back to the top. The scroll reset is explicit because
`scrollToIndex` on the newest row parks the list one header-height down: that
row sits at index 1, since index 0 is its date-group header, and `align:
"auto"` resolves to `"start"`.

**The arrow keys no longer select the first row when nothing is selected.**
`moveSelection` clamped `findIndex`'s `-1` straight into `items[0]`, so ↑ sent
you to the newest entry — the opposite of what ↑ means everywhere else in the
list. With no row to step away from, both directions are now inert.

**A browser-preview fixture id no longer reaches the real app.** The
first-paint preselect preferred `MOCK_SELECTED_ID` unconditionally, so a cold
start landed on whichever SQLite row happened to own id 2. It turned out to
be unreachable in any case — it pointed at a pinned row, which the history
rail filters out — so it is gone rather than fenced. Screenshots were always
driven by `?select=`, which is unaffected.

## [0.1.3] — 2026-10-05

A visual pass over the whole panel. No behaviour changed, and no Rust
changed: every edit is a class name or a token value.

### Changed

**The panel now draws from closed design scales.** Spacing, type, radius,
shadow and ink each come from a fixed set declared at the top of
`src/index.css`, instead of being picked per component.

- Spacing is on a 4px grid (4/8/12/16/24/32/48/64), with a 2px sub-step
  allowed for icon-level gaps only. The seventeen values that sat on no
  scale — `h-10.5`, `h-6.5`, `h-5.25`, `w-46.5`, `pt-17` and the rest — are
  gone, and `src/` now scans clean for arbitrary values.
- Type is three steps: `micro` 11, `caption` 12, `body` 13. That replaces
  five sizes, two of which were used exactly once (`11.5px`, `10px`).
- Radius is `sm`/`md`/`lg`/`xl` plus one panel corner, and shadows are three
  levels — `seg` < `menu` < `panel`. The segmented control's hand-written
  shadow is a token now, like the other two.
- The two menus that had drifted to different row heights (34px and 32px)
  share one.

**Metadata labels are a step down from their values.** The `Information`
rows had label and value at the same 13px, separated only by tone, so a row
read as one sentence — "Source Chrome" — and squinting told you nothing.
The label is now `micro`/faint above a `body`/ink value: two type steps
apart, and the hierarchy survives with all colour removed.

**The search field got a glyph.** It has no container of its own — it is a
transparent input inside a drag strip — so nothing marked it as a search
box, and it sat next to a fully-bordered dropdown at a different weight.

### Fixed

- **Three ink tones fell short of WCAG AA (4.5:1).** `faint` measured
  **2.54:1** on white and **2.87:1** on the dark panel, and it was carrying
  real copy rather than decoration: empty-state bodies, input placeholders,
  the "Add a note" affordance, keycap legends. `muted` was **4.39:1** in
  dark mode. All three are now the darkest set that still separates —
  light **17.0 / 6.6 / 4.6**, dark **12.8 / 6.7 / 4.8**.
- **The destructive-action red was 3.91:1**, as a hard-coded hex in two
  places. It is a `--rc-danger` token now, at 5.49:1 (light) and 5.17:1
  (dark). The dark accent was 3.87:1 and is now 5.02:1.
- **The detail preview ended flush against the rule above the note.** The
  bottom inset now matches the top one. The centred colour swatch is
  unaffected, because `inset-0` resolves against the padding box, which
  padding does not change.

## [0.1.2] — 2026-10-03

### Added

**Real application icons in the detail panel** — the `Source` row now shows
the actual icon of the app an entry came from, instead of a coloured square
with the app's initial.

- Resolved on the Rust side from the bundle id already stored on every entry,
  via `NSWorkspace`. Rendered to a 32px PNG and cached under `icons/` in the
  data dir, so an entry copied weeks ago shows its icon even though that app
  has long since quit — a running-app lookup cannot do that.
- **Falls back to the initial tile, as before, when the icon cannot be had**:
  the app is not installed, never declared a bundle id, or the data dir was
  emptied after the entry was first viewed. These are ordinary states, not
  errors, and both paths are drawn by the same component, so the row never
  shows a broken image.
- Resolved on read rather than on capture. AppKit's icon service wants the
  main thread and capture runs on a worker thread, so capturing is not an
  option — and the list only ever shows the initial anyway.
- Icons are never deleted. They belong to an app, not to an entry: removing
  one would strip the icon from every other entry from the same app.
- The asset protocol scope is unchanged. Real icons live in `/Applications`,
  outside it, and widening it would let the webview read any file in any app
  bundle — while `iconForFile:` needs nothing but a path. Writing the PNG on
  the Rust side keeps the scope exactly as narrow as it was.
- The file name is a hash of the bundle id, not the id itself. A bundle id is
  declared by the app that carries it, so putting one in a path unfiltered
  would be a path traversal; hashing makes that impossible by construction.

## [0.1.1] — 2026-10-03

### Fixed

- **`scripts/reinstall.sh` reported an Accessibility grant on a machine that
  had none.** Without the permission the app logs one line containing both
  "armed" and "inert", and the script tested for "armed" first — so it matched
  the untrusted wording, printed "Accessibility granted", and never reached the
  branch that would have said otherwise. The app now prints one machine-readable
  `[hotkey] accessibility=granted|denied|unavailable` line at startup and the
  script matches only that, so the untrusted and unavailable cases are told
  apart from a real grant. A "could not install the monitors at all" case is
  reported separately, because that one is not a permissions problem.

### Changed

- Releases are now published when the tag is pushed, instead of being staged as
  a draft that has to be published by hand. `ci.yml` already builds the unsigned
  bundle on every push, so the draft only added a step that was easy to forget.

## [0.1.0] — 2026-10-03

First public release.

### Added

**Notes** — a per-entry line of your own text

- Attach a note to any history entry, edited in place or with `Cmd+D`, saved
  with `Cmd+Return`. `Escape` cancels.
- Notes are searchable alongside titles, so an entry can be found by why you
  kept it rather than by what it contained.
- A pencil mark in the list shows which entries have one. Deliberately an icon
  rather than a text line: the list is virtualised against a fixed 40px row
  height, and showing the note would make row height depend on content.
- Re-copying the same content does not clobber the note — the dedupe path
  touches only the two timestamps, and a test pins that behaviour.

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
- No `LSMinimumSystemVersion` is set, so macOS decides the effective floor.
  Treat the target as "a recent macOS on Apple Silicon".
- Release builds are code-signed but not notarized, so first launch requires
  right-click → Open, or clearing the quarantine attribute.
- Double-tap-Option and auto-paste require the Accessibility permission, which
  is bound to the app's designated requirement — see
  [SECURITY.md](SECURITY.md#security-relevant-design-notes).

### Removed

- The `base64` dependency. It had no call sites; removing it avoids shipping
  0.23's default-on `simd-unsafe` AVX2/NEON kernels for a crate never invoked.
  Copies at 0.21.7 and 0.22.1 remain in the tree, pulled in transitively by
  `tauri`, `wry`, `plist` and others.

[Unreleased]: https://github.com/navms/clipboard/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/navms/clipboard/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/navms/clipboard/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/navms/clipboard/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/navms/clipboard/releases/tag/v0.1.0
