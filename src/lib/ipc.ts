import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import {
  openUrl as openerOpenUrl,
  revealItemInDir,
} from "@tauri-apps/plugin-opener";
import type {
  ClipDetail,
  ClipListItem,
  ClipQuery,
  PasteResult,
  Settings,
  TypeFilter,
} from "../types/clip";
import { MOCK_ITEMS, mockDetail } from "./mock";

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Thin, typed wrapper over the Rust command surface.
 *
 * Every call has a browser-preview fallback that operates on an in-memory
 * copy of the mock fixtures, so the entire UI (search, filter, pin, delete,
 * Cmd+K menu) is fully interactive via `pnpm dev` without a Rust toolchain.
 */

const delay = (ms = 20) => new Promise((r) => setTimeout(r, ms));

/** Turns a stored filesystem path into an <img>-loadable URL. */
function toAsset(path?: string | null): string | null {
  if (!path) return null;
  if (path.startsWith("data:") || path.startsWith("http")) return path;
  return isTauri ? convertFileSrc(path) : path;
}

function hydrate<T extends ClipDetail>(item: T): T {
  return {
    ...item,
    thumbPath: toAsset(item.thumbPath),
    imagePath: toAsset(item.imagePath),
    sourceIcon: toAsset(item.sourceIcon),
  };
}

// --- browser-preview state ------------------------------------------------

let previewItems: ClipListItem[] = MOCK_ITEMS.map((i) => ({ ...i }));
const previewDeleted = new Set<number>();

let previewSettings: Settings = {
  theme: "system",
  hotkey: "Alt+Super+V",
  autostart: false,
  maxItems: 500,
};

function matchesFilter(kind: ClipListItem["kind"], filter: TypeFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "text":
      return kind === "text";
    case "images":
      return kind === "image";
    case "files":
      return kind === "file";
    case "links":
      return kind === "link";
    case "colors":
      return kind === "color";
  }
}

function applyQuery(q: ClipQuery): ClipListItem[] {
  const search = (q.search ?? "").trim().toLowerCase();
  return previewItems
    .filter((i) => !previewDeleted.has(i.id))
    .filter((i) => matchesFilter(i.kind, q.filter ?? "all"))
    .filter((i) => (search ? i.title.toLowerCase().includes(search) : true))
    .sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.createdAt - a.createdAt;
    })
    .slice(0, q.limit ?? 500);
}

// --- public API -----------------------------------------------------------

export async function listClips(query: ClipQuery = {}): Promise<ClipListItem[]> {
  if (isTauri) {
    const rows = await invoke<ClipListItem[]>("list_clips", { query });
    return rows.map(hydrate);
  }
  await delay();
  return applyQuery(query);
}

export async function getClipDetail(id: number): Promise<ClipDetail | null> {
  if (isTauri) {
    const row = await invoke<ClipDetail | null>("get_clip_detail", { id });
    return row ? hydrate(row) : null;
  }
  await delay(10);
  if (previewDeleted.has(id)) return null;
  return mockDetail(id);
}

export async function pinClip(id: number, pinned: boolean): Promise<void> {
  if (isTauri) return invoke("pin_clip", { id, pinned });
  previewItems = previewItems.map((i) => (i.id === id ? { ...i, pinned } : i));
}

/**
 * Attaches a note to an entry, or clears it with `null`.
 *
 * The blank-to-`null` rule is duplicated here for the browser preview only;
 * the real path delegates to Rust, which normalises before writing. Both ends
 * must agree because the list renders a "has a note" affordance off this
 * value, and a preview-only empty string would show a pencil that the Tauri
 * build never displays.
 */
export async function setDescription(
  id: number,
  description: string | null,
): Promise<void> {
  const normalised =
    description?.trim() === "" || description == null
      ? null
      : (description as string).trim();
  if (isTauri) return invoke("set_description", { id, description: normalised });
  previewItems = previewItems.map((i) =>
    i.id === id ? { ...i, description: normalised } : i,
  );
}

export async function deleteClip(id: number): Promise<void> {
  if (isTauri) return invoke("delete_clip", { id });
  previewDeleted.add(id);
}

export async function clearClips(keepPinned: boolean): Promise<void> {
  if (isTauri) return invoke("clear_clips", { keepPinned });
  for (const item of previewItems) {
    if (keepPinned && item.pinned) continue;
    previewDeleted.add(item.id);
  }
}

/**
 * Puts the clip on the pasteboard, then fires Cmd+V into the target app.
 *
 * The panel only stays open when we could not paste - see `PasteResult`.
 */
export async function pasteClip(id: number): Promise<PasteResult> {
  if (isTauri) return invoke<PasteResult>("paste_clip", { id });
  await delay(30);
  return {
    targetApp: "Google Chrome",
    autoPasted: true,
    needsAccessibility: false,
  };
}

/** Pasteboard only: the Cmd+K "Copy" action, where no app receives a keystroke. */
export async function copyClip(id: number): Promise<void> {
  if (isTauri) return invoke("copy_clip", { id });
  await delay(30);
}

export async function hidePanel(): Promise<void> {
  if (isTauri) return invoke("hide_panel");
}

/**
 * Starts a window move, from `mousedown` on the panel's chrome.
 *
 * Rust-side `start_dragging` rather than Tauri's `data-tauri-drag-region`
 * attribute: that one also routes double-click to `internal_toggle_maximize`,
 * and a fixed-size launcher panel has nothing to maximise. Owning the command
 * also keeps the window-moving primitive out of the capability file.
 */
export async function beginPanelDrag(): Promise<void> {
  if (isTauri) return invoke("begin_panel_drag");
}

/** Forgets a dragged position and re-centres the panel on the cursor's display. */
export async function resetPanelPosition(): Promise<void> {
  if (isTauri) return invoke("reset_panel_position");
}

/** Deep-links to System Settings -> Privacy & Security -> Accessibility. */
export async function openAccessibilitySettings(): Promise<void> {
  if (isTauri) return invoke("open_accessibility_settings");
  await delay(10);
}

/**
 * Hands a URL to the OS.
 *
 * `window.open(url, "_blank")` looks like the obvious call and is a **silent
 * no-op inside the webview** - WKWebView has no `createWebViewWith` handler,
 * so Tauri returns null and nothing at all happens. The opener plugin goes
 * through LaunchServices instead, which is what actually opens a browser.
 */
export async function openUrl(url: string): Promise<void> {
  if (isTauri) return openerOpenUrl(url);
  window.open(url, "_blank", "noopener");
}

/** Shows the given paths in Finder, selecting them. */
export async function revealItems(paths: string[]): Promise<void> {
  if (!paths.length) return;
  if (isTauri) return revealItemInDir(paths);
  await delay(10);
}

export async function getSettings(): Promise<Settings> {
  if (isTauri) return invoke<Settings>("get_settings");
  return { ...previewSettings };
}

export async function updateSettings(patch: Partial<Settings>): Promise<Settings> {
  if (isTauri) return invoke<Settings>("update_settings", { patch });
  previewSettings = { ...previewSettings, ...patch };
  return { ...previewSettings };
}
