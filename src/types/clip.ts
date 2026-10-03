/**
 * Shared clipboard domain types.
 *
 * These mirror `src-tauri/src/models.rs` one-to-one; keep both sides in
 * sync when a field changes on either end.
 */

export type ClipKind = "text" | "image" | "color" | "link" | "file";

/** Filter buckets exposed by the "All Types" dropdown. */
export type TypeFilter = "all" | "text" | "images" | "files" | "links" | "colors";

/**
 * Which rail the sidebar is showing.
 *
 * `pinned` is the shelf: everything deliberately kept, in its own view.
 * `history` is the timeline, and it treats pinned entries differently
 * depending on whether you are browsing or searching — see `deriveItems`.
 */
export type HistoryView = "history" | "pinned";

/** Lightweight shape used to render a row in the history list. */
export interface ClipListItem {
  id: number;
  kind: ClipKind;
  /** Display title - derived on the Rust side (first line / file name / ...). */
  title: string;
  /** Colour swatch for `kind: "color"`. */
  color?: string | null;
  /** Absolute path to the cached thumbnail (images only). */
  thumbPath?: string | null;
  /** Absolute path to the original asset (images only). */
  imagePath?: string | null;
  sourceApp?: string | null;
  sourceBundle?: string | null;
  /**
   * A free-text note the user attached to this entry.
   *
   * `null` means "no note" — blank input is normalised on write, so this is
   * never an empty string. The list renders a "has a note" affordance from
   * this field, which is why it lives here rather than only on `ClipDetail`.
   */
  description?: string | null;
  pinned: boolean;
  createdAt: number;
  updatedAt: number;
}

/** Full record, loaded lazily for the detail pane. */
export interface ClipDetail extends ClipListItem {
  contentText?: string | null;
  contentHtml?: string | null;
  width?: number | null;
  height?: number | null;
  byteSize?: number | null;
  characters?: number | null;
  words?: number | null;
  /**
   * Asset URL of the source application's icon, resolved on the Rust side.
   *
   * Detail-only on purpose: it is not a stored field but a lookup answered
   * when the pane opens, and putting it on `ClipListItem` would have the list
   * pay for an icon it never draws.
   */
  sourceIcon?: string | null;
}

export interface ClipQuery {
  search?: string;
  filter?: TypeFilter;
  limit?: number;
}

export interface Settings {
  theme: "system" | "light" | "dark";
  hotkey: string;
  autostart: boolean;
  maxItems: number;
}

/** Outcome of a "Paste to <app>" attempt. */
export interface PasteResult {
  targetApp: string | null;
  /** True when a synthetic Cmd+V was actually sent to the target app. */
  autoPasted: boolean;
  /**
   * True when auto-paste was refused because the Accessibility permission is
   * missing. The panel stays open in that case, so the UI can say so.
   */
  needsAccessibility: boolean;
}

export interface DateGroup<T> {
  label: string;
  items: T[];
}
