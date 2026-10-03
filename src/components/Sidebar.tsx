import { Pin } from "lucide-react";
import { useApp } from "../stores/useApp";
import { EntryList } from "./EntryList";
import { ViewTabs } from "./ViewTabs";

/** Fixed-width left rail holding the grouped clipboard history. */
export function Sidebar() {
  return (
    <aside className="flex w-72 shrink-0 flex-col border-r border-divider bg-app">
      <ViewTabs />
      <EntryList />
      <PinnedHint />
    </aside>
  );
}

/**
 * Tells you where your pinned entries went.
 *
 * Under the History rail they are hidden on purpose, and a list that is
 * quietly missing rows reads as data loss. The hint sits *outside* the scroll
 * area rather than at the end of the list, so it cannot be scrolled out of
 * sight — which is exactly when it would have been useful.
 *
 * It retires itself in three cases, all of them honest: you are already on the
 * Pinned rail, a query is active (search reaches pinned rows, so nothing is
 * hidden while you type), or there is nothing pinned to account for.
 */
function PinnedHint() {
  const view = useApp((s) => s.view);
  const query = useApp((s) => s.query);
  const pool = useApp((s) => s.pool);

  const count = pool.reduce((n, item) => (item.pinned ? n + 1 : n), 0);
  if (view !== "history" || query.trim() || count === 0) return null;

  return (
    <div className="mx-2 mb-2 flex shrink-0 items-start gap-1.75 rounded-lg bg-hover px-2.5 py-1.75 text-[11.5px] leading-[1.45] text-muted">
      <Pin className="mt-px h-3.25 w-3.25 shrink-0 text-faint" strokeWidth={2} />
      <span>
        {count} pinned {count === 1 ? "entry" : "entries"} kept in the Pinned
        tab — History stays chronological.
      </span>
    </div>
  );
}
