import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { observeElementRect, useVirtualizer } from "@tanstack/react-virtual";
import { useApp } from "../stores/useApp";
import { railGroups } from "../lib/format";
import type { ClipListItem } from "../types/clip";
import { EntryRow } from "./EntryRow";
import { ScrollArea } from "./common/ScrollArea";

const HEADER_H = 32;
const ROW_H = 40;

type Row =
  | { type: "header"; label: string; count: number | null; key: string }
  | { type: "item"; item: ClipListItem; key: string };

/**
 * The last viewport rect worth believing in, and an observer that refuses to
 * report anything smaller than belief.
 *
 * A hidden window is not a zero-sized window, but WebKit reports it as one:
 * the `ResizeObserver` delivers a zero rect on the way out and is not
 * guaranteed to deliver again on the way back, and a freshly mounted
 * virtualiser's synchronous first read can land while the waking window
 * still reports zero. Every one of those zeros ends up in the virtualiser's
 * cached `scrollRect`, and a zero-height viewport makes the range
 * calculation bail out and return nothing — the blank rail.
 *
 * Zero is never a size worth caching (the panel is 760x480, fixed), so this
 * wrapper drops it and keeps the last real reading. A virtualiser fed
 * through here can never strand its `scrollRect` at zero: either a real
 * measurement has already landed and is kept, or none has and `getSize()`
 * falls back to `initialRect` — which is why the seed below has to be
 * non-zero. It only needs to be in the right ballpark: it steadies the
 * windowing until the first real measurement arrives, and the panel's size
 * never changes after that.
 */
let lastViewportRect = { width: 272, height: 320 };

const observeViewportRect: typeof observeElementRect = (instance, cb) =>
  observeElementRect(instance, (rect) => {
    if (rect.height <= 0) return;
    lastViewportRect = rect;
    cb(rect);
  });

/**
 * Tracks the scroll container's height, and re-reads it whenever the panel
 * comes back on screen.
 *
 * The virtualiser cannot do this for itself. It reads the viewport height from
 * a rect cached at mount, refreshes it only when the scroll element's
 * *identity* changes, and a resident panel keeps the same element across every
 * hide and show — so the cached height stays whatever the hidden window
 * reported, which is zero. A zero-height viewport makes the range calculation
 * bail out and return nothing, and the rail renders blank.
 *
 * Recreating the virtualiser is the reliable cure, which is what `key` is for:
 * a fresh instance re-reads the container's true size and recomputes its range
 * from scratch. Remounting the rail is cheap — the row components are cheap —
 * and it does not disturb anything outside the list, so trading it for a rail
 * that always paints is worth it.
 *
 * `rAF` for the re-read because the moment a summoned panel is shown the DOM
 * has not necessarily been laid out yet; by the next frame it has, and the real
 * height is available.
 */
function useViewportHeight(ref: React.RefObject<HTMLDivElement | null>, epoch: number) {
  const [height, setHeight] = useState(0);

  const measure = useCallback(() => {
    const el = ref.current;
    setHeight(el ? el.clientHeight : 0);
  }, [ref]);

  // Mount, and every panel summon. The timeout is a backstop for the case the
  // observer misses: WebKit does not reliably report a `display: none` →
  // visible transition, so without it the rail can wait indefinitely for a
  // resize that never arrives.
  useEffect(() => {
    if (epoch === 0) return;
    let raf = 0;
    let timer = 0;
    const read = () => {
      measure();
      // One more read after layout has certainly settled. A window coming back
      // from `win.hide()` can be measured before AppKit has finished sizing
      // it, and that first reading is wrong in the one direction that strands
      // the list at zero.
      timer = window.setTimeout(measure, 60);
    };
    raf = requestAnimationFrame(read);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(timer);
    };
  }, [epoch, measure]);

  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") {
      measure();
      return;
    }
    const observer = new ResizeObserver(() => measure());
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  }, [ref, measure]);

  return height;
}

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
  const listEpoch = useApp((s) => s.listEpoch);
  const select = useApp((s) => s.select);
  const pasteSelected = useApp((s) => s.pasteSelected);
  const loading = useApp((s) => s.loading);

  const scrollRef = useRef<HTMLDivElement>(null);
  const viewportHeight = useViewportHeight(scrollRef, listEpoch);

  const rows = useMemo<Row[]>(() => {
    const groups = railGroups(items, view);

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

  // Set when a summon has just walked the rail back to the top, and read by
  // `Rail` to make its scroll-into-view stand down for that one pass.
  //
  // A ref rather than state on purpose: it has to be readable *during* the same
  // commit that renders the fresh selection, so it cannot be a value the next
  // render would observe. A version using state would either miss the window
  // or take an extra render to clear.
  //
  // `summonedForId` is the selection the stand-down belongs to. The window
  // closes when the selection becomes something *else* — the arrows moving it,
  // or a click — because that is a genuine navigation intent rather than the
  // selection the summon itself produced. Comparing ids, rather than clearing
  // on any change, is what keeps the summon from disarming itself: the reader
  // runs in the same commit as one that would clear it.
  const summonedForIdRef = useRef<number | null | undefined>(undefined);

  // The rail is remounted on a summon so its virtualiser re-measures the
  // viewport, and remounting resets the offset to zero on its own. A fresh
  // instance reads the scroll element where it actually is, which the
  // surviving one cannot: its cached size is still whatever the hidden window
  // reported.
  useEffect(() => {
    if (listEpoch === 0) return;
    // Paired with the selection the summon is about to land on. That is
    // `items[0]` — the same row `refresh()` picks once the summon has cleared
    // the selection, and the newest one in the rail — so recording it here is
    // what lets `Rail` recognise the arrival and stand down, rather than
    // arming a blanket "skip the next scroll" that would also swallow a real
    // one.
    //
    // Read from `items` rather than `rows`: this runs on the commit *before*
    // `refresh()` lands, so `rows` still describes the outgoing list, while
    // `items[0]` is what both `refresh()` and this effect agree the new
    // selection will be.
    summonedForIdRef.current = items.length ? items[0].id : null;
  }, [listEpoch, items]);

  return (
    <ScrollArea scrollRef={scrollRef} className="px-2 py-2">
      {rows.length === 0 ? (
        <EmptyState loading={loading} view={view} searching={!!query.trim()} />
      ) : (
        // Keyed on the viewport height so a panel that comes back at a
        // different size gets a virtualiser that has actually measured it. A
        // zero-height rail — the state a hidden window leaves behind — renders
        // nothing at all, and nothing short of a scroll brings it back.
        <Rail
          key={`${viewportHeight}:${listEpoch}`}
          rows={rows}
          selectedId={selectedId}
          scrollRef={scrollRef}
          select={select}
          pasteSelected={pasteSelected}
          standDownFor={summonedForIdRef}
        />
      )}
    </ScrollArea>
  );
}

interface RailProps {
  rows: Row[];
  selectedId: number | null;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  select: (id: number) => void;
  pasteSelected: () => void;
  /** The selection the last summon parked the rail on; see `EntryList`. */
  standDownFor: React.RefObject<number | null | undefined>;
}

/**
 * The windowing half of the rail, split out so it can be *remounted*.
 *
 * A virtualiser caches the scroll element's size when it mounts and only
 * refreshes that cache when the element's identity changes. This panel is a
 * resident window that gets hidden rather than unmounted, so the element is
 * always the same one — and the cache keeps whatever the hidden window
 * reported, which is zero. The range calculation treats a zero-height viewport
 * as "nothing fits" and returns no rows, leaving the rail blank until some
 * unrelated event (a scroll, a resize) happens to force a recount.
 *
 * Remounting is the direct fix: a new instance measures the container as it
 * actually is. Nothing outside this component holds the instance, so the cost
 * is a few row components, and it buys a rail that always paints.
 */
function Rail({
  rows,
  selectedId,
  scrollRef,
  select,
  pasteSelected,
  standDownFor,
}: RailProps) {
  // Stable identity is load-bearing: `EntryRow` is `memo`ised, and an inline
  // arrow passed as `onPaste` was a fresh function every render — which is
  // every scroll frame and every selection move — so the memo never held and
  // the whole visible window re-rendered on each one. `pasteSelected` comes
  // from the store and is referentially stable, so this callback is too.
  const handlePaste = useCallback(() => pasteSelected(), [pasteSelected]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => (rows[index].type === "header" ? HEADER_H : ROW_H),
    getItemKey: (index) => rows[index].key,
    overscan: 10,
    // See `observeViewportRect` above: a hidden-then-woken window reports a
    // zero viewport and may never report again; the zero is dropped and the
    // last real rect (or the seed) stands in, so `outerSize` can never be
    // zero and the rail can never go blank over a stale measurement.
    observeElementRect: observeViewportRect,
    initialRect: lastViewportRect,
    // Row heights are fixed, and this panel is hidden and shown rather than
    // unmounted. Without caching, the `ResizeObserver` that measures each row
    // reports 0 for all of them while the window is hidden, which wipes every
    // measurement; the list then re-estimates on the way back in and the rows
    // visibly jump. Caching keeps the known sizes and only re-measures rows
    // that actually change.
    useCachedMeasurements: true,
  });

  // Keep the keyboard-selected row inside the viewport.
  useEffect(() => {
    if (selectedId == null) return;
    // Stands down right after a summon: that pass has already parked the rail
    // at the top, and the newest row — the one now selected — is visible
    // there. Scrolling it "into view" would undo that, and it would show: the
    // row sits at index 1, since index 0 is its date-group header, and
    // `align: "auto"` resolves to `"start"`, parking the rail one
    // header-height down with "Today" scrolled off the top.
    if (standDownFor.current === selectedId) return;
    const index = rows.findIndex(
      (r) => r.type === "item" && r.item.id === selectedId,
    );
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "auto" });
  }, [selectedId, rows, virtualizer, standDownFor]);

  // A fresh instance starts at offset zero, which is the reset the summon wants
  // — but only the first row group would be in view if the viewport were
  // mismeasured, so assert it once now that the size is known good.
  useEffect(() => {
    if (selectedId == null) return;
    if (standDownFor.current !== selectedId) return;
    virtualizer.scrollToOffset(0);
  }, [selectedId, virtualizer, standDownFor]);

  return (
    <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((virtualRow) => {
        const row = rows[virtualRow.index];
        return (
          <div
            key={virtualRow.key}
            className="absolute left-0 top-0 w-full"
            style={{ transform: `translateY(${virtualRow.start}px)` }}
          >
            {row.type === "header" ? (
              // De-emphasised group heading: the same micro step as every
              // other section label, one tone lighter than the row titles
              // it introduces. The count is subordinate to the heading, so
              // it drops a tone rather than a size.
              <div className="flex h-8 items-center gap-2 px-2 text-micro font-semibold text-muted">
                {row.label}
                {row.count != null && (
                  <span className="rounded-full bg-hover px-2 py-px text-micro font-medium text-faint">
                    {row.count}
                  </span>
                )}
              </div>
            ) : (
              <EntryRow
                item={row.item}
                selected={row.item.id === selectedId}
                onSelect={select}
                onPaste={handlePaste}
              />
            )}
          </div>
        );
      })}
    </div>
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
      <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
        <p className="text-body font-semibold text-muted">Loading clipboard…</p>
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

  // Two levels only: the title carries the state, the body says how to get out
  // of it. Anything more competes with a list that is about to replace this
  // whole block.
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
      <p className="text-body font-semibold text-muted">{title}</p>
      <p className="text-caption text-faint">{body}</p>
    </div>
  );
}
