import type { ClipDetail, ClipListItem } from "../types/clip";
import { countWords } from "./format";

/**
 * Browser-preview fixtures.
 *
 * Fixed rows with hand-picked titles and timestamps, so the rendered UI is
 * reproducible between runs. Extra rows are appended under "Yesterday" so
 * date grouping and the type filters have something to bite on, and three
 * rows arrive pinned so the Pinned rail has a shelf to show. Never used
 * when running inside Tauri.
 *
 * A few rows also carry a `description`, so the note affordances (the pencil
 * in the list, the filled editor in the detail pane) and description-aware
 * search are all exercisable without a Rust toolchain.
 */

/**
 * Real app icons, captured from the macOS icon service, for the preview only.
 *
 * The Tauri build resolves these itself (`appicon::resolve`); here they are
 * static files so `pnpm dev` can exercise the `<img>` branch of `AppIcon`
 * without a Rust toolchain. Only apps installed on the machine that captured
 * them made it in, which is the point: the rows whose bundle id is missing
 * from this table fall through to the initial tile exactly as an uninstalled
 * app would in the real build.
 */
const MOCK_ICONS: Record<string, string> = {
  "com.google.Chrome": "/mock-icons/chrome.png",
  "com.apple.Notes": "/mock-icons/notes.png",
  "com.apple.finder": "/mock-icons/finder.png",
};

function todayAt(h: number, m: number, s = 0): number {
  const d = new Date();
  d.setHours(h, m, s, 0);
  return d.getTime();
}

function daysAgo(n: number, h = 11, m = 20): number {
  const d = new Date();
  d.setDate(d.getDate() - n);
  d.setHours(h, m, 0, 0);
  return d.getTime();
}

/** A tiny SVG that reads as a "screenshot" at 20x20 px. */
function miniShot(bg: string, accent: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="46" height="30" viewBox="0 0 46 30">
    <rect width="46" height="30" rx="3" fill="${bg}"/>
    <rect x="3.5" y="3.5" width="15" height="23" rx="2" fill="${accent}" opacity="0.32"/>
    <rect x="22" y="4" width="20" height="3.5" rx="1.75" fill="${accent}" opacity="0.55"/>
    <rect x="22" y="11" width="20" height="3" rx="1.5" fill="${accent}" opacity="0.3"/>
    <rect x="22" y="17" width="12" height="3" rx="1.5" fill="${accent}" opacity="0.3"/>
  </svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

const SHOT_LIGHT = miniShot("#f6f7f9", "#3b82f6");
const SHOT_DARK = miniShot("#2b2b2f", "#8b5cf6");

let seq = 0;
function row(
  partial: Omit<ClipListItem, "id" | "pinned" | "updatedAt"> & {
    pinned?: boolean;
  },
): ClipListItem {
  seq += 1;
  return {
    id: seq,
    pinned: false,
    updatedAt: partial.createdAt,
    ...partial,
  };
}

export const MOCK_ITEMS: ClipListItem[] = [
  row({
    kind: "image",
    title: "Image (751×477)",
    thumbPath: SHOT_LIGHT,
    sourceApp: "PixPin",
    sourceBundle: "com.pixpin.mac",
    createdAt: todayAt(20, 51, 38),
  }),
  row({
    kind: "text",
    title: "Clipboard",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    createdAt: todayAt(20, 49, 9),
    pinned: true,
  }),
  row({
    kind: "text",
    title: "capture while unfocused",
    sourceApp: "Notes",
    sourceBundle: "com.apple.Notes",
    description: "失焦期间也能抓到，用来对照 focus 变化时序。",
    createdAt: todayAt(20, 46, 12),
  }),
  row({
    kind: "color",
    title: "#3366FF",
    color: "#3366FF",
    sourceApp: "Figma",
    sourceBundle: "com.figma.Desktop",
    createdAt: todayAt(20, 44, 51),
    pinned: true,
  }),
  row({
    kind: "link",
    title: "https://example.com/focus-test",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    description: "复现丢帧用的最小页面，验收时按这里写的两步走。",
    createdAt: todayAt(20, 42, 3),
  }),
  row({
    kind: "text",
    title: "live capture one",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    createdAt: todayAt(20, 39, 47),
  }),
  row({
    kind: "text",
    title: "second capture after focus change",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    createdAt: todayAt(20, 37, 20),
  }),
  row({
    kind: "text",
    title: "first capture",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    createdAt: todayAt(20, 35, 2),
  }),
  row({
    kind: "text",
    title: "focus lost test",
    sourceApp: "Google Chrome",
    sourceBundle: "com.google.Chrome",
    createdAt: todayAt(20, 33, 44),
  }),
  row({
    kind: "image",
    title: "Image (752×478)",
    thumbPath: SHOT_DARK,
    sourceApp: "CleanShot X",
    sourceBundle: "pl.maketheweb.cleanshotx",
    createdAt: daysAgo(1, 18, 12),
  }),
  row({
    kind: "file",
    title: "clipboard-tech-design.md",
    sourceApp: "Finder",
    sourceBundle: "com.apple.finder",
    createdAt: daysAgo(1, 9, 5),
    pinned: true,
  }),
];

const LONG_TEXT = "Clipboard";

export function mockDetail(id: number): ClipDetail {
  const base = MOCK_ITEMS.find((i) => i.id === id) ?? MOCK_ITEMS[0];
  const text =
    base.kind === "text"
      ? base.title
      : base.kind === "link"
        ? base.title
        : base.kind === "color"
          ? base.color ?? base.title
          : base.kind === "file"
            ? `/Users/me/docs/${base.title}`
            : null;

  const detail: ClipDetail = {
    ...base,
    contentText: text,
    characters: text ? text.length : null,
    words: text ? countWords(text) : null,
    sourceIcon: base.sourceBundle ? (MOCK_ICONS[base.sourceBundle] ?? null) : null,
  };

  if (base.kind === "image") {
    const isFirst = base.id === MOCK_ITEMS[0].id;
    detail.width = isFirst ? 751 : 752;
    detail.height = isFirst ? 477 : 478;
    detail.byteSize = isFirst ? 154_000 : 210_400;
  }

  return detail;
}

export const MOCK_SELECTED_ID = MOCK_ITEMS[1].id; // "Clipboard"
export const MOCK_LONG_TEXT = LONG_TEXT;
