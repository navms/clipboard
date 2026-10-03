import { useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useApp } from "../stores/useApp";
import { groupByDate } from "../lib/format";
import type { ClipListItem, DateGroup } from "../types/clip";
import { EntryRow } from "./EntryRow";
import { ScrollArea } from "./common/ScrollArea";

const HEADER_H = 32;
const ROW_H = 40;

type Row =
  | { type: "header"; label: string; count: number | null; key: string }
  | { type: "item"; item: ClipListItem; key: string };

/**
 * Left rail: grouped, virtualised history list.
 *
 * Groups are flattened into a single row array (headers included) so the
 * virtualiser can window them as one list, which keeps scrolling smooth at
 * thousands of entries.
 */
export function EntryList() {
  const items = useApp((s) => s.items);
  const view = useApp((s) => s.view);
  const query = useApp((s) => s.query);
  const selectedId = useApp((s) => s.selectedId);
  const select = useApp((s) => s.select);
  const pasteSelected = useApp((s) => s.pasteSelected);
  const loading = useApp((s) => s.loading);

  const scrollRef = useRef<HTMLDivElement>(null);

  const rows = useMemo<Row[]>(() => {
    // The Pinned rail is a shelf, not a timeline: every row is pinned by
    // definition, so date grouping would only ever produce one meaningless
    // bucket per day. It gets a single header — which also carries the count,
    // the same as the Pinned group does over in History.
    const groups: DateGroup<ClipListItem>[] =
      view === "pinned"
        ? items.length
          ? [{ label: "Pinned", items }]
          : []
        : groupByDate(items);

    const out: Row[] = [];
    for (const group of groups) {
      const pinned = group.label === "Pinned";
      out.push({
        type: "header",
        label: group.label,
        count: pinned ? group.items.length : null,
        key: `h:${group.label}`,
      });
      for (const item of group.items) {
        out.push({ type: "item", item, key: `i:${item.id}` });
      }
    }
    return out;
  }, [items, view]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => (rows[index].type === "header" ? HEADER_H : ROW_H),
    getItemKey: (index) => rows[index].key,
    overscan: 10,
  });

  // Keep the keyboard-selected row inside the viewport.
  useEffect(() => {
    if (selectedId == null) return;
    const index = rows.findIndex(
      (r) => r.type === "item" && r.item.id === selectedId,
    );
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "auto" });
  }, [selectedId, rows, virtualizer]);

  return (
    <ScrollArea scrollRef={scrollRef} className="px-2 py-2">
      {rows.length === 0 ? (
        <EmptyState loading={loading} view={view} searching={!!query.trim()} />
      ) : (
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const row = rows[virtualRow.index];
            return (
              <div
                key={virtualRow.key}
                className="absolute left-0 top-0 w-full"
                style={{ transform: `translateY(${virtualRow.start}px)` }}
              >
                {row.type === "header" ? (
                  <div className="flex h-8 items-center gap-1.5 px-2 text-[11px] font-semibold tracking-[0.01em] text-muted">
                    {row.label}
                    {row.count != null && (
                      <span className="rounded-full bg-hover px-1.5 py-px text-[10px] font-medium text-faint">
                        {row.count}
                      </span>
                    )}
                  </div>
                ) : (
                  <EntryRow
                    item={row.item}
                    selected={row.item.id === selectedId}
                    onSelect={select}
                    onPaste={() => pasteSelected()}
                  />
                )}
              </div>
            );
          })}
        </div>
      )}
    </ScrollArea>
  );
}

/**
 * Four different kinds of nothing, and they are not interchangeable.
 *
 * "Nothing pinned yet" has to say how to fix it, because an empty shelf is a
 * state a new user has no way to have caused on purpose. A bare "Nothing here
 * yet" on the Pinned rail would read as a broken tab.
 */
function EmptyState({
  loading,
  view,
  searching,
}: {
  loading: boolean;
  view: "history" | "pinned";
  searching: boolean;
}) {
  let title = "Nothing here yet";
  let body = "Copy something and it will show up here.";

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
        <p className="text-[13px] font-medium text-muted">
          Loading clipboard…
        </p>
      </div>
    );
  }

  if (view === "pinned") {
    if (searching) {
      title = "No pinned entries match";
      body = "Try a different search.";
    } else {
      title = "Nothing pinned yet";
      body = "Pin an entry from the history and it will stay here.";
    }
  } else if (searching) {
    title = "Nothing matches";
    body = "Try a different search.";
  }

  return (
    <div className="flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
      <p className="text-[13px] font-medium text-muted">{title}</p>
      <p className="text-[12px] leading-relaxed text-faint">{body}</p>
    </div>
  );
}
