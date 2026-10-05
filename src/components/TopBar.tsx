import { useEffect, useRef } from "react";
import { Search, X } from "lucide-react";
import { beginPanelDrag, resetPanelPosition } from "../lib/ipc";
import { useApp } from "../stores/useApp";
import { TypeFilter } from "./TypeFilter";

/**
 * Whether a press landed on the bar's own surface rather than on a control.
 *
 * The window has `decorations: false`, so there is no title bar to grab: the
 * bar *is* the handle. But everything interactive has to keep behaving
 * normally, which is why this walks up the tree looking for something that
 * owns its own clicks. It mirrors the check Tauri's own drag-region script
 * makes, so the bar behaves the way the rest of the platform does.
 */
function isBarSurface(target: EventTarget | null): boolean {
  const el = target as Element | null;
  if (!el || typeof el.closest !== "function") return false;

  return !el.closest(
    "input, textarea, select, button, a, label, [role], [tabindex], [contenteditable='true']",
  );
}

export function TopBar() {
  const query = useApp((s) => s.query);
  const setQuery = useApp((s) => s.setQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  /** Where the second press of a double-click landed. */
  const secondPress = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const focus = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };
    window.addEventListener("rc:focus-search", focus);
    return () => window.removeEventListener("rc:focus-search", focus);
  }, []);

  return (
    <header
      onMouseDown={(e) => {
        if (e.button !== 0 || !isBarSurface(e.target)) return;

        // The second press of a double-click must not start a drag: AppKit's
        // drag loop would swallow the mouseup that tells us the click was a
        // plain one, and the reset would never fire.
        if (e.detail === 2) {
          secondPress.current = { x: e.clientX, y: e.clientY };
          return;
        }

        // Suppresses the text cursor flashing over the bar's own surface.
        e.preventDefault();
        void beginPanelDrag();
      }}
      onMouseUp={(e) => {
        const origin = secondPress.current;
        secondPress.current = null;

        if (e.button !== 0 || e.detail !== 2 || !origin) return;
        // A double-click that turned into a drag is a drag. Only a click that
        // stayed put counts as "put the panel back".
        if (e.clientX !== origin.x || e.clientY !== origin.y) return;
        if (!isBarSurface(e.target)) return;

        void resetPanelPosition();
      }}
      className="flex h-14 shrink-0 items-center gap-2 border-b border-divider px-3"
    >
      {/* The field has no container of its own — a transparent input in a
          drag strip — so the glyph is the only thing that says "this is a
          search box". Without it the left half of the bar reads as empty
          chrome next to a fully-bordered dropdown. */}
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <Search
          className="h-4 w-4 shrink-0 text-faint"
          strokeWidth={1.75}
          aria-hidden
        />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape" && query) {
              e.stopPropagation();
              setQuery("");
            }
          }}
          placeholder="Type to filter entries..."
          spellCheck={false}
          autoComplete="off"
          className="h-8 w-full min-w-0 bg-transparent px-2 text-body text-ink outline-none placeholder:text-faint"
        />
        {query && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => {
              setQuery("");
              inputRef.current?.focus();
            }}
            className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-black/8 text-muted hover:bg-black/16 dark:bg-white/12 dark:hover:bg-white/20"
          >
            <X className="h-3 w-3" strokeWidth={2.5} />
          </button>
        )}
      </div>

      <TypeFilter />
    </header>
  );
}
