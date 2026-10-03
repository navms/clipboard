import type { ClipListItem, DateGroup } from "../types/clip";

/** Renders an epoch timestamp as "Today at 20:49:09". */
export function formatCopied(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const day = startOfDay(d);
  const today = startOfDay(now);
  const diffDays = Math.round((today - day) / 86_400_000);

  const time = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(d);

  if (diffDays === 0) return `Today at ${time}`;
  if (diffDays === 1) return `Yesterday at ${time}`;

  const date = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
  }).format(d);
  return `${date} at ${time}`;
}

/** 154 KB / 1.2 MB - decimal units, two significant-ish digits. */
export function formatBytes(bytes?: number | null): string {
  if (bytes == null) return "—";
  if (bytes < 1000) return `${bytes} B`;
  const kb = bytes / 1000;
  if (kb < 1000) return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)} KB`;
  const mb = kb / 1000;
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

export function formatDimensions(w?: number | null, h?: number | null): string {
  if (w && h) return `${w}×${h}`;
  return "—";
}

export function countWords(text?: string | null): number {
  if (!text) return 0;
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
}

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/**
 * Buckets a flat, newest-first list into Today / Yesterday / Earlier
 * groups. The Rust side already returns rows ordered by
 * `pinned DESC, created_at DESC`, so pinned rows naturally float to the
 * top group.
 */
export function groupByDate(items: ClipListItem[]): DateGroup<ClipListItem>[] {
  const now = new Date();
  const today = startOfDay(now);
  const yesterday = today - 86_400_000;

  const buckets: DateGroup<ClipListItem>[] = [
    { label: "Pinned", items: [] },
    { label: "Today", items: [] },
    { label: "Yesterday", items: [] },
    { label: "Earlier", items: [] },
  ];

  for (const item of items) {
    if (item.pinned) {
      buckets[0].items.push(item);
      continue;
    }
    const day = startOfDay(new Date(item.createdAt));
    if (day >= today) buckets[1].items.push(item);
    else if (day >= yesterday) buckets[2].items.push(item);
    else buckets[3].items.push(item);
  }

  return buckets.filter((b) => b.items.length > 0);
}

/** Truncates the tail of a long URL so rows stay on one line. */
export function shortenUrl(url: string, max = 42): string {
  const stripped = url.replace(/^https?:\/\//, "").replace(/\/$/, "");
  if (stripped.length <= max) return stripped;
  return `${stripped.slice(0, max - 1)}…`;
}
