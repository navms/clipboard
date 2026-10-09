# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.6] — 2026-10-09

The rail stops re-rendering on every scroll, a note opened from the keyboard
stops erasing itself, and the footer shows the app's own icon.

### Fixed

**The note editor no longer wipes a note when opened from the keyboard.** The
draft was only seeded when the editor was opened by a click. Opened from
<kbd>⌘</kbd><kbd>D</kbd> or the <kbd>⌘</kbd><kbd>K</kbd> menu it was not, so a
filled note came up as an empty box — and the blur-to-save then committed that
empty draft, silently erasing it. The draft is seeded however the editor opens.

**A double pin toggles again.** The pin action read the entry's state and then
awaited the write, so a second press inside that window sent the same value and
the pin never came back. It now flips locally first and lets a refresh reconcile.

**Startup reconciliation no longer deletes pinned entries.** It dropped any row
whose asset files had vanished, without the `pinned` exemption that `prune` and
`clear` both honour — so a pinned screenshot whose file had gone quietly
disappeared. Pinned rows are now left alone.

**The post-delete selection steps to a neighbour** instead of clearing itself
when the deleted row was not in the current view.

### Changed

**The history list stops re-rendering on every scroll.** `EntryRow`'s paste
handler was rebuilt on every render, which defeated its `memo` — every scroll
frame and every arrow press re-rendered the whole visible window. The handler is
now stable, and search folds each row once when the index is built instead of on
every keystroke.

**Capture and query work moved off the hot path.** The watcher moves a large
paste's text out of the snapshot instead of cloning it, `list_clips` runs off the
main thread, and a persist queue that backs up past a threshold now logs once.
The queue stays unbounded on purpose: a dropped capture would be lost user data.

**The footer carries the app's own icon.** It is the bundle icon rather than a
redrawn mark, with a drop-shadow so its white tile still reads on the white bar.

### Removed

- A dead mock export, a `matchMedia` listener that was re-added on every init,
  and the actions-menu keydown listener's re-registration on every cursor move.

## [0.1.5] — 2026-10-07

The rail stops going blank when the panel comes back, and it stops jumping on
the way there. Both symptoms were the same underlying staleness, and 0.1.4 fixed
only half of it — that release made every summon reset the rail, but the reset
could not take effect on a virtualiser that still believed the window had zero
height.

### Fixed

**The rail no longer paints blank after reopening the panel.** The windowing
half of the list caches the scroll container's height when it mounts and
refreshes that cache only when the element's *identity* changes. This panel is
a resident window that gets hidden rather than unmounted, so the element is
always the same one, and the cache keeps whatever the hidden window reported —
zero. A zero-height viewport makes the range calculation bail out and return
nothing, so the rail stayed blank until some unrelated event forced a recount.
In practice: blank on reopening, cleared by scrolling.

Remounting the windowing half is the direct cure, and the only one available:
the cached rect is written once at mount, so a new instance measuring the
container as it actually is is the sole way to refresh it. The remount is keyed
on the measured viewport height plus the summon epoch, so a rail that returns at
a different size is rebuilt and one that did not is left alone. A fresh instance
also starts at offset zero, which the reset the summon wants gets for free.

**The rail no longer jumps from the old position to the top on its way back.**
The reset waited for the clip query to resolve before moving the scroll offset,
which cost a frame painted at the previous position. The offset is DOM state
and needs no data to move, so it now resets in the same commit as the rest of
the per-session state.

### Changed

- The rail is split into an inner component so it can be remounted, and its
  scroll-into-view now stands down for the selection the summon just landed on.
  Declaring the reset second was never sufficient: the two effects do not run in
  the same commit, so the ordering was never going to win.

## [0.1.4] — 2026-10-06

A summoned panel lands where it should, and the panel's own idea of "where
that is" no longer drifts. Three fixes behind one cause — the panel is never
torn down between openings — plus two dependency patches.

### Changed

**Dependencies.** Vite 8.0.16 → 8.3.2, lucide-react 1.49.0 → 1.50.0, and
@tauri-apps/api / cli 2.12.0 → 2.12.1, via Dependabot.

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

[Unreleased]: https://github.com/navms/clipboard/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/navms/clipboard/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/navms/clipboard/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/navms/clipboard/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/navms/clipboard/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/navms/clipboard/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/navms/clipboard/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/navms/clipboard/releases/tag/v0.1.0
