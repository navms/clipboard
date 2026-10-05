import { cn } from "../lib/cn";
import { useApp } from "../stores/useApp";
import type { HistoryView } from "../types/clip";

const TABS: { value: HistoryView; label: string; shortcut: string }[] = [
  { value: "history", label: "History", shortcut: "⌘1" },
  { value: "pinned", label: "Pinned", shortcut: "⌘2" },
];

/**
 * The rail's History / Pinned switch.
 *
 * A segmented control rather than another dropdown: these are two peer views
 * of the same data, and the shelf needs a fixed address you can aim at, not a
 * transient filter state. It costs ~34px of list height, which is the price of
 * pinned entries no longer having to float into the timeline to stay findable.
 *
 * The `title` doubles as the only affordance for the keyboard shortcut; the
 * bottom bar already documents the global ones, and a second shortcut legend
 * up here would read as clutter in a 288px rail.
 */
export function ViewTabs() {
  const view = useApp((s) => s.view);
  const setView = useApp((s) => s.setView);

  return (
    <div
      role="tablist"
      aria-label="Clipboard view"
      className="mx-2 mt-2 mb-1 flex shrink-0 gap-0.5 rounded-lg bg-seg-track p-0.5"
    >
      {TABS.map((tab) => {
        const selected = tab.value === view;
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={selected}
            title={`${tab.label} (${tab.shortcut})`}
            onClick={() => setView(tab.value)}
            className={cn(
              "h-6 flex-1 rounded-md text-caption font-medium transition-colors",
              selected
                ? // `shadow-seg` is the bottom of the elevation scale: a chip
                  // resting on its own recessed track, nothing more.
                  "bg-seg-active text-ink shadow-seg"
                : "bg-transparent text-muted hover:text-ink",
            )}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
