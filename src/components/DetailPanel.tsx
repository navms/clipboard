import { useApp } from "../stores/useApp";
import type { ClipDetail } from "../types/clip";
import { ClipPreview } from "./detail/ClipPreview";
import { InfoSection } from "./detail/InfoSection";
import { NoteSection } from "./detail/NoteSection";
import { ScrollArea } from "./common/ScrollArea";

/**
 * Right pane. Content preview flexes to fill the space with the editable
 * `Note` and the read-only `Information` block pinned to the bottom edge, so
 * the metadata stays visible while the preview takes whatever height is left.
 */
export function DetailPanel() {
  const selectedId = useApp((s) => s.selectedId);
  const items = useApp((s) => s.items);
  const detail = useApp((s) => s.detail);
  const loading = useApp((s) => s.loading);

  const base = items.find((i) => i.id === selectedId) ?? null;
  const clip: ClipDetail | null = base ? { ...base, ...(detail ?? {}) } : null;

  if (!clip) {
    return (
      <section className="flex min-w-0 flex-1 flex-col items-center justify-center gap-1 bg-app px-8 text-center">
        <p className="text-[13px] font-medium text-muted">
          {loading ? "Loading…" : "No entry selected"}
        </p>
        {!loading && (
          <p className="text-[12px] text-faint">
            Pick an entry from the list to inspect it.
          </p>
        )}
      </section>
    );
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-app">
      {/* `relative` is the anchor for previews that cover the whole pane
          (the colour swatch uses `absolute inset-0`). */}
      <ScrollArea className="relative px-4 pt-3.5">
        <ClipPreview clip={clip} />
      </ScrollArea>

      <div className="shrink-0 border-t border-divider pt-2">
        <NoteSection />
        <InfoSection clip={clip} />
      </div>
    </section>
  );
}
