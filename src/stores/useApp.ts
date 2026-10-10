import Fuse from "fuse.js";
import { create } from "zustand";
import * as ipc from "../lib/ipc";
import { railOrder } from "../lib/format";
import type {
  ClipDetail,
  ClipListItem,
  HistoryView,
  Settings,
  TypeFilter,
} from "../types/clip";

type Theme = Settings["theme"];

interface AppState {
  /** Filter-applied rows straight from the backend. */
  pool: ClipListItem[];
  /** What the list renders: `pool` narrowed by the search box. */
  items: ClipListItem[];
  detail: ClipDetail | null;
  loading: boolean;

  query: string;
  filter: TypeFilter;
  view: HistoryView;
  selectedId: number | null;

  actionsOpen: boolean;
  typeFilterOpen: boolean;

  /**
   * Id of the entry whose note editor is open, or `null`.
   *
   * The editor's own draft text stays local to `NoteSection`; only the
   * intent lives here, so the keyboard and the Cmd+K menu can start an edit
   * without owning the component.
   */
  noteEditingId: number | null;

  /**
   * Incremented every time the panel is summoned, *after* the fresh page
   * has landed. `EntryList` watches it to walk the rail back to the top.
   *
   * A counter rather than a boolean: the panel can be shown any number of
   * times, and "already true" would not re-trigger the effect the second
   * time. Same reasoning as `noteEditingId` being an id and not a flag.
   */
  listEpoch: number;

  /** Foreground app captured when the panel was summoned; drives the
   *  "Paste to <app>" hint in the bottom bar. */
  targetApp: string | null;

  /** Set when a paste was refused for want of the Accessibility permission.
   *  The bottom bar swaps its normal content for an actionable hint. */
  pasteBlocked: boolean;

  themeSetting: Theme;
  resolvedTheme: "light" | "dark";

  init: () => Promise<void>;
  refresh: () => Promise<void>;
  summonPanel: (targetApp: string | null) => Promise<void>;
  setQuery: (q: string) => void;
  setFilter: (f: TypeFilter) => void;
  setView: (v: HistoryView) => void;
  setTypeFilterOpen: (open: boolean) => void;
  setActionsOpen: (open: boolean) => void;
  beginNoteEdit: () => void;
  endNoteEdit: () => void;
  select: (id: number | null) => void;
  moveSelection: (delta: number) => void;
  togglePin: (id: number) => Promise<void>;
  setDescription: (id: number, description: string | null) => Promise<void>;
  remove: (id: number) => Promise<void>;
  clearAll: (keepPinned: boolean) => Promise<void>;
  pasteSelected: () => Promise<void>;
  copySelected: () => Promise<void>;
  openSelectedLink: () => Promise<void>;
  revealSelectedFiles: () => Promise<void>;
  openSelected: () => Promise<void>;
  dismissPasteBlocked: () => void;
  openAccessibilitySettings: () => Promise<void>;
  setTheme: (t: Theme) => Promise<void>;
  cycleTheme: () => Promise<void>;
}

function systemTheme(): "light" | "dark" {
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/** `?theme=dark` wins for the browser preview; otherwise the setting rules. */
function readThemeOverride(): Theme | null {
  if (typeof window === "undefined") return null;
  const v = new URLSearchParams(window.location.search).get("theme");
  return v === "light" || v === "dark" || v === "system" ? v : null;
}

/**
 * Preview-only affordance: `?open=filter` mounts with the type dropdown
 * expanded, and `?open=actions` opens the Cmd+K menu. No effect inside Tauri.
 */
function readOpenOverride(): "filter" | "actions" | null {
  if (typeof window === "undefined") return null;
  const v = new URLSearchParams(window.location.search).get("open");
  return v === "filter" || v === "actions" ? v : null;
}

/**
 * Preview-only: `?select=color` (or any clip kind, or a numeric id) pins the
 * detail pane to that entry, so each per-kind preview can be screenshotted
 * without driving the mouse. No effect inside Tauri.
 */
function readSelectOverride(pool: ClipListItem[]): number | null {
  if (typeof window === "undefined") return null;
  const v = new URLSearchParams(window.location.search).get("select");
  if (!v) return null;
  const byKind = pool.find((i) => i.kind === v);
  if (byKind) return byKind.id;
  const n = Number(v);
  return Number.isInteger(n) ? n : null;
}

/**
 * Preview-only: `?paste=blocked` mounts with the Accessibility hint already
 * showing. The real trigger needs a machine that has *not* granted the
 * permission, which is not something a screenshot run can arrange.
 */
function readPasteOverride(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("paste") === "blocked";
}

/**
 * Preview-only: `?edit` opens the note editor on the selected entry, so the
 * editing state can be screenshotted without driving the keyboard. No effect
 * inside Tauri.
 */
function readEditOverride(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).has("edit");
}

function applyTheme(setting: Theme): "light" | "dark" {
  const resolved = setting === "system" ? systemTheme() : setting;
  if (typeof document !== "undefined") {
    document.documentElement.classList.toggle("dark", resolved === "dark");
  }
  return resolved;
}

/**
 * The OS colour-scheme listener, held at module scope.
 *
 * `init` runs twice under React StrictMode, and a bare `addEventListener` in
 * there installed a second copy of the listener each time with no way to drop
 * the first. Keeping the pair here lets a re-run remove the previous one
 * before adding its replacement.
 */
let themeQuery: MediaQueryList | null = null;
let onSystemThemeChange: (() => void) | null = null;

// --------------------------------------------------------------------------
// Client-side search
// --------------------------------------------------------------------------

/**
 * Fuzzy index over the loaded page of history. Rebuilt only when the pool
 * changes, so typing stays synchronous and IPC-free; at multi-thousand-row
 * scale this moves to SQLite FTS5.
 */
let fuse: Fuse<ClipListItem> | null = null;

/**
 * Lower-cased `title` / `description` per entry, keyed by id.
 *
 * Kept beside the Fuse index so the substring pass in `searchPool` never
 * re-folds the whole page on every keystroke: at 500 rows an unbuffered pass
 * was ~1000 `toLowerCase` allocations per character typed. `rebuildIndex` is
 * the one place a fresh `pool` arrives (`refresh`, `setDescription`, `remove`),
 * so the two indices can never drift apart.
 */
let lowerIndex = new Map<
  number,
  { title: string; description: string | null }
>();

function rebuildIndex(pool: ClipListItem[]) {
  fuse = new Fuse(pool, {
    // `description` is searchable too: a note is often the only place the
    // reason for keeping something is written down, so finding the entry by
    // its note is the whole point of having one.
    keys: ["title", "description"],
    threshold: 0.38,
    ignoreLocation: true,
    minMatchCharLength: 1,
  });
  lowerIndex = new Map(
    pool.map((item) => [
      item.id,
      {
        title: item.title.toLowerCase(),
        description: item.description?.toLowerCase() ?? null,
      },
    ]),
  );
}

function searchPool(pool: ClipListItem[], query: string): ClipListItem[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return pool;

  // Substring hits first: this is what makes CJK queries behave, since
  // Fuse's tokenizer splits on whitespace and would miss them entirely.
  // The note is checked on the same footing as the title — for a CJK note,
  // this substring path is the only thing that will ever match it.
  const exact = pool.filter((item) => {
    const lower = lowerIndex.get(item.id);
    // The fallback only fires if a caller ever hands us an id the index has
    // not seen; folding that one row is cheap insurance against silently
    // dropping a hit.
    const title = lower ? lower.title : item.title.toLowerCase();
    const description = lower
      ? lower.description
      : item.description?.toLowerCase() ?? null;
    return title.includes(needle) || description?.includes(needle) === true;
  });
  const seen = new Set(exact.map((item) => item.id));

  const fuzzy = (fuse?.search(query) ?? [])
    .map((hit) => hit.item)
    .filter((item) => !seen.has(item.id));

  return [...exact, ...fuzzy];
}

/**
 * Turns the loaded page into the rows the rail actually renders.
 *
 * This is where the two views stop being the same list:
 *
 *   pinned  - the shelf. Only pinned entries, and the search box still
 *             narrows within it.
 *
 *   history - the timeline, and its handling of pinned rows deliberately
 *             splits browse from search:
 *               * browsing stays chronological, with pinned entries living
 *                 in their own view rather than floating into the top group;
 *               * searching reaches everything, because a saved entry you
 *                 cannot find is the one failure mode a launcher must not
 *                 have. `pool` arrives ordered `pinned DESC`, and
 *                 `searchPool` preserves that order, so pinned hits land on
 *                 top and `groupByDate` files them under "Pinned" for free.
 *
 * Nothing here hits IPC: the page already holds pinned rows (SQL sorts them
 * first, so the limit can never crowd them out), which is what lets the rail
 * tabs swap instantly instead of flashing a loading state.
 */
function deriveItems(
  pool: ClipListItem[],
  query: string,
  view: HistoryView,
): ClipListItem[] {
  if (view === "pinned") return searchPool(pool.filter((i) => i.pinned), query);
  if (!query.trim()) return pool.filter((i) => !i.pinned);
  return searchPool(pool, query);
}

/**
 * The payload text of the selected entry, but only when it is of `kind`.
 *
 * Kind-bound actions (opening a link, revealing a file) apply to exactly one
 * kind, so the guard lives here instead of in every caller. The `detail.id`
 * re-check matters: `detail` is fetched asynchronously and still holds the
 * previous row for a moment after the selection moves.
 */
function selectedContent(
  state: AppState,
  kind: ClipListItem["kind"],
): string | null {
  const item = state.items.find((i) => i.id === state.selectedId);
  if (!item || item.kind !== kind) return null;

  const detail = state.detail?.id === item.id ? state.detail : null;
  const text = (detail?.contentText ?? "").trim();
  return text || null;
}

export const useApp = create<AppState>((set, get) => ({
  pool: [],
  items: [],
  detail: null,
  loading: true,

  query: "",
  filter: "all",
  view: "history",
  selectedId: null,

  actionsOpen: false,
  typeFilterOpen: false,
  noteEditingId: null,

  listEpoch: 0,

  targetApp: null,
  pasteBlocked: false,
  themeSetting: "system",
  resolvedTheme: "light",

  async init() {
    const settings = await ipc.getSettings();
    const override = readThemeOverride();
    const setting = override ?? settings.theme;
    const resolved = applyTheme(setting);
    set({ themeSetting: setting, resolvedTheme: resolved });

    if (typeof window !== "undefined" && window.matchMedia) {
      if (themeQuery && onSystemThemeChange) {
        themeQuery.removeEventListener("change", onSystemThemeChange);
      }
      themeQuery = window.matchMedia("(prefers-color-scheme: dark)");
      onSystemThemeChange = () => {
        if (get().themeSetting === "system") {
          set({ resolvedTheme: applyTheme("system") });
        }
      };
      themeQuery.addEventListener("change", onSystemThemeChange);
    }

    await get().refresh();

    const openOverride = readOpenOverride();
    if (openOverride === "filter") set({ typeFilterOpen: true });
    if (openOverride === "actions") set({ actionsOpen: true });
    if (readPasteOverride()) set({ pasteBlocked: true });

    // Preselect a sensible row so the detail pane is never empty on first
    // paint.
    //
    // `?select=` is the only way to aim the preview at a specific entry (see
    // `readSelectOverride`); everything else just takes the newest row. There
    // used to be a `MOCK_SELECTED_ID` fallback here for the fixture id, but it
    // pointed at a *pinned* row, which `deriveItems` filters out of this rail
    // — so it could never match. Inside Tauri it was worse than dead: ids come
    // from SQLite, so a cold start would land on whichever row happened to own
    // that number. The screenshots were always driven by `?select=`.
    const items = get().items;
    const forced = readSelectOverride(items);
    const preferred =
      (forced != null ? items.find((i) => i.id === forced) : undefined) ??
      items[0];
    if (preferred) get().select(preferred.id);

    // `beginNoteEdit` reads `selectedId`, so it has to run after the selection
    // above has landed.
    if (readEditOverride()) get().beginNoteEdit();
  },

  async refresh() {
    const { filter, view } = get();
    set({ loading: true });

    const pool = await ipc.listClips({ filter, limit: 500 });
    rebuildIndex(pool);

    // The page is fetched unfiltered by view on purpose: History's search has
    // to be able to reach pinned rows, so pinned-ness is partitioned here
    // rather than in SQL.
    const items = deriveItems(pool, get().query, view);
    set({ pool, items, loading: false });

    if (!items.some((i) => i.id === get().selectedId)) {
      get().select(items.length ? items[0].id : null);
    }
  },

  /**
   * Puts the panel back the way a summon should find it.
   *
   * The panel is never torn down between openings — Rust only toggles
   * `win.show()` / `win.hide()`, so the React tree, this store, the DOM
   * scroll offset and the virtualiser instance all survive. Every piece of
   * per-session state therefore has to be cleared *here* or it becomes the
   * next opening's initial state, which is how the panel used to come back
   * on whichever row was selected last time.
   *
   * `listEpoch` is bumped in the same commit as the reset itself, *before*
   * the `refresh()` await. The scroll offset is DOM state and needs no data
   * to move, so waiting for the query only produces a visible flash: the rail
   * renders once at the old offset, then jumps. Bumping early means the rail
   * is already at the top by the time the first frame is painted.
   *
   * Bumping *early* is safe for the other effect in `EntryList` — the one
   * that scrolls the selected row into view. It runs on the commit where
   * `refresh()` lands the new selection, and being declared first, the reset
   * still has the last word on that commit.
   */
  async summonPanel(targetApp) {
    set({
      query: "",
      filter: "all",
      // Land on the timeline, same as the type filter: a freshly
      // summoned panel should look the same every time.
      view: "history",
      actionsOpen: false,
      typeFilterOpen: false,
      // A note editor belongs to the row it was opened on. `NoteSection`
      // closes itself when the selection moves *off* that row, so leaving
      // this set would re-open the editor with its old draft whenever the
      // newest row happened to be the one being edited.
      noteEditingId: null,
      targetApp,
      // Announced, not applied: the rail's scroll offset is DOM state this
      // store cannot reach, so `EntryList` reads this and walks the list back
      // to the top. Same commit as the reset above, so the rail never paints
      // at the stale offset.
      listEpoch: get().listEpoch + 1,
    });

    // Cleared rather than re-pointed: it is the guard in `refresh` that
    // picks the newest row, and it only fires when nothing is selected.
    // Routed through `select` instead of a bare field because that is the
    // one place that already knows a null selection has to take the
    // fetched detail down with it.
    get().select(null);

    await get().refresh();
  },

  setQuery(q) {
    const { pool, selectedId, view } = get();
    const items = deriveItems(pool, q, view);
    set({ query: q, items });
    if (!items.some((i) => i.id === selectedId)) {
      get().select(items.length ? items[0].id : null);
    }
  },

  setFilter(f) {
    set({ filter: f, typeFilterOpen: false });
    void get().refresh();
  },

  /**
   * Switches rail tabs.
   *
   * Deliberately re-derives from the `pool` already in memory instead of
   * calling `refresh`: the page is a superset of both views, so the swap is
   * free and instant. Going back to IPC would put a stale-list flash between
   * the click and the new rows, which is worse than the round trip is cheap.
   */
  setView(v) {
    const { pool, query, selectedId } = get();
    const items = deriveItems(pool, query, v);
    set({ view: v, items });
    if (!items.some((i) => i.id === selectedId)) {
      get().select(items.length ? items[0].id : null);
    }
  },

  setTypeFilterOpen(open) {
    set({ typeFilterOpen: open });
  },

  setActionsOpen(open) {
    set({ actionsOpen: open });
  },

  /**
   * Opens the note editor for the selected entry.
   *
   * Lives here rather than inside `NoteSection` so that the two entry points
   * outside the pane — the Cmd+D shortcut and the Cmd+K menu item — reach the
   * same editor the click target drives, instead of each growing its own.
   * A monotonic counter is what the editor watches: the menu can be opened
   * again on an entry that is already being edited, and a boolean that was
   * already `true` would not re-trigger the effect.
   */
  beginNoteEdit() {
    if (get().selectedId == null) return;
    set({ noteEditingId: get().selectedId });
  },

  endNoteEdit() {
    if (get().noteEditingId == null) return;
    set({ noteEditingId: null });
  },

  select(id) {
    if (id === null) {
      set({ selectedId: null, detail: null });
      return;
    }
    set({ selectedId: id, detail: null });
    void ipc.getClipDetail(id).then((detail) => {
      if (get().selectedId === id) set({ detail });
    });
  },

  moveSelection(delta) {
    const { items, view, selectedId } = get();
    if (!items.length) return;
    // Stepped over `railOrder`, not `items`: the two are the same list only
    // while browsing. Once a query is in play they are not — `items` is in
    // match order (substring hits, then Fuse scores) and the rail re-buckets it
    // by date, so walking `items` sent ↑/↓ to rows that were nowhere near the
    // highlighted one on screen.
    const order = railOrder(items, view);
    const idx = order.findIndex((i) => i.id === selectedId);
    // With nothing selected there is no row to step away from, so both
    // directions are inert. Letting the clamp below absorb `idx === -1`
    // would send ↑ to the *first* row, which reads as a jump to the newest
    // entry — the opposite of what ↑ means everywhere else in the list.
    if (idx === -1) return;
    const next = Math.min(Math.max(idx + delta, 0), order.length - 1);
    if (next === idx) return;
    get().select(order[next].id);
  },

  async togglePin(id) {
    const item = get().pool.find((i) => i.id === id);
    if (!item) return;
    const next = !item.pinned;

    // Flip locally *before* the round trip. Cmd+P is easy to hit twice, and
    // reading `pinned` off the pool and then awaiting the write left the
    // second press looking at the pre-first-press value — so it sent the same
    // one and the pin never came back. The optimistic flip makes the second
    // press see the first press's result. `refresh()` reconciles with the
    // server afterwards, which also unwinds the flip if the write failed.
    const pool = get().pool.map((i) =>
      i.id === id ? { ...i, pinned: next } : i,
    );
    set({ pool, items: deriveItems(pool, get().query, get().view) });

    await ipc.pinClip(id, next);
    await get().refresh();
  },

  /**
   * Writes a note and patches the two local lists in place.
   *
   * Deliberately not a `refresh()`: this fires on every blur of the note
   * editor, and refetching the whole page to echo back one string the caller
   * already has would make the caret jump and the list flash. The Fuse index
   * *is* rebuilt, because a note is searchable and the index would otherwise
   * hand back a stale score for the entry that just changed.
   */
  async setDescription(id, description) {
    await ipc.setDescription(id, description);

    const pool = get().pool.map((i) =>
      i.id === id ? { ...i, description } : i,
    );
    rebuildIndex(pool);
    const items = deriveItems(pool, get().query, get().view);
    set({ pool, items });

    // The detail pane merges the list row over the fetched detail, so it
    // picks the new value up from `items` without a second round trip.
    const detail = get().detail;
    if (detail && detail.id === id) {
      set({ detail: { ...detail, description } });
    }
  },

  async remove(id) {
    const { items, view } = get();
    // Also over `railOrder`: `idx` is only ever used to pick the row that takes
    // the deleted one's place, and that has to be a visual neighbour.
    const order = railOrder(items, view);
    const idx = order.findIndex((i) => i.id === id);
    await ipc.deleteClip(id);

    const pool = get().pool.filter((i) => i.id !== id);
    rebuildIndex(pool);
    // Re-derived through `deriveItems`, never `searchPool` directly: the rail's
    // rows are view-dependent, and this was the one recompute site that skipped
    // the view. Deleting on the Pinned tab therefore dropped the pinned
    // partition and refilled the shelf with the whole history.
    const remaining = deriveItems(pool, get().query, view);
    set({ pool, items: remaining });

    // Same order as `idx` above, or the fallback selects an unrelated row.
    // Clamped at 0 as well as at the end: `idx` is -1 whenever the deleted row
    // was not in the current view, and `rail[-1]` is `undefined`, which
    // silently cleared the selection instead of stepping to a neighbour.
    const rail = railOrder(remaining, view);
    const fallback = rail[Math.max(0, Math.min(idx, rail.length - 1))];
    get().select(fallback ? fallback.id : null);
  },

  async clearAll(keepPinned) {
    await ipc.clearClips(keepPinned);
    await get().refresh();
  },

  async pasteSelected() {
    const id = get().selectedId;
    if (id == null) return;
    const result = await ipc.pasteClip(id);
    // `needsAccessibility` means the backend left the panel up on purpose:
    // the clip is on the pasteboard, but no keystroke went anywhere.
    set({
      targetApp: result.targetApp ?? get().targetApp,
      pasteBlocked: result.needsAccessibility,
    });
  },

  async copySelected() {
    const id = get().selectedId;
    if (id == null) return;
    await ipc.copyClip(id);
  },

  async openSelectedLink() {
    const url = selectedContent(get(), "link");
    if (url) await ipc.openUrl(url);
  },

  async revealSelectedFiles() {
    const text = selectedContent(get(), "file");
    if (!text) return;
    const paths = text
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
    await ipc.revealItems(paths);
  },

  /**
   * Cmd+O - runs the selected entry's primary action, whichever kind that is.
   *
   * "Open" here is macOS's Cmd+O sense of *acting on* the thing: a link goes to
   * the browser, a file is revealed in Finder. Text, images and colours have
   * no such action and fall through, which is what lets the shortcut be global
   * without being a trap. It used to be links-only, on the theory that a global
   * Cmd+O would fight the webview; there is no document in this webview for Cmd+O to
   * have meant anything else, so that guard protected nothing and only made the
   * shortcut's availability unpredictable.
   *
   * The two branches deliberately reuse the same actions the preview's rows
   * call, so the shortcut and the click can never drift apart.
   */
  async openSelected() {
    const kind = get().items.find((i) => i.id === get().selectedId)?.kind;
    if (kind === "link") await get().openSelectedLink();
    else if (kind === "file") await get().revealSelectedFiles();
  },

  dismissPasteBlocked() {
    set({ pasteBlocked: false });
  },

  async openAccessibilitySettings() {
    await ipc.openAccessibilitySettings();
    // The user is about to leave for System Settings; holding the hint up
    // would just get in the way when they come back.
    set({ pasteBlocked: false });
  },

  async setTheme(t) {
    const resolved = applyTheme(t);
    set({ themeSetting: t, resolvedTheme: resolved });
    await ipc.updateSettings({ theme: t });
  },

  async cycleTheme() {
    const order: Theme[] = ["system", "light", "dark"];
    const cur = get().themeSetting;
    const next = order[(order.indexOf(cur) + 1) % order.length];
    await get().setTheme(next);
  },
}));
