import { Fragment, useEffect, useMemo, useState } from "react";
import {
  ClipboardPaste,
  Copy,
  Eraser,
  Pencil,
  Pin,
  PinOff,
  SunMoon,
  Trash2,
} from "lucide-react";
import { cn } from "../lib/cn";
import { useApp } from "../stores/useApp";
import type { ClipDetail } from "../types/clip";

interface Action {
  id: string;
  label: string;
  icon: typeof Copy;
  run: () => void;
  danger?: boolean;
}

/**
 * Cmd+K command menu.
 *
 * Two tiers: actions bound to the selected entry on top, app-level actions
 * (theme, clear history) pinned below a divider. The app-level tier never
 * depends on the selection, so it renders even with an empty history, which
 * is what keeps Clear History reachable now that the tray menu no longer
 * carries it.
 */
export function ActionsMenu() {
  const open = useApp((s) => s.actionsOpen);
  const setOpen = useApp((s) => s.setActionsOpen);
  const selectedId = useApp((s) => s.selectedId);
  const items = useApp((s) => s.items);
  const detail = useApp((s) => s.detail);
  const targetApp = useApp((s) => s.targetApp);

  const [cursor, setCursor] = useState(0);

  const clip: ClipDetail | null = useMemo(() => {
    const base = items.find((i) => i.id === selectedId) ?? null;
    return base ? { ...base, ...(detail ?? {}) } : null;
  }, [items, selectedId, detail]);

  const groups = useMemo<Action[][]>(() => {
    // `getState()` keeps these callbacks off the render path: they only run
    // on user intent, so subscribing to the whole store would be wasteful.
    const s = useApp.getState;

    // App-level tier: scope is the app itself, not a row, so it is always
    // present and always last.
    const appLevel: Action[] = [
      {
        id: "theme",
        label: "Toggle Theme",
        icon: SunMoon,
        run: () => void s().cycleTheme(),
      },
      {
        id: "clear",
        label: "Clear History (keep pinned)",
        icon: Eraser,
        run: () => void s().clearAll(true),
      },
    ];

    if (!clip) return [appLevel];

    const entry: Action[] = [
      {
        id: "paste",
        label: `Paste to ${targetApp ?? "the active app"}`,
        icon: ClipboardPaste,
        run: () => void s().pasteSelected(),
      },
      {
        id: "copy",
        label: "Copy",
        icon: Copy,
        run: () => void s().copySelected(),
      },
      {
        id: "pin",
        label: clip.pinned ? "Unpin" : "Pin to Top",
        icon: clip.pinned ? PinOff : Pin,
        run: () => void s().togglePin(clip.id),
      },
      {
        id: "note",
        label: clip.description ? "Edit Note" : "Add Note",
        icon: Pencil,
        run: () => s().beginNoteEdit(),
      },
    ];

    // Deliberately *no* kind-specific entries here.
    //
    // "Open Link" and "Reveal in Finder" used to appear only for some kinds,
    // which made the menu a different shape every time and put the action far
    // from the thing it acts on. They now live next to the content itself
    // (see `ClipPreview`), so this list is the same shape for every entry.
    entry.push({
      id: "delete",
      label: "Delete",
      icon: Trash2,
      danger: true,
      run: () => void s().remove(clip.id),
    });

    return [entry, appLevel];
  }, [clip, targetApp]);

  /** Keyboard/mouse cursor walks this flattened view; the divider is not a
   *  stop, it only exists at paint time. */
  const actions = useMemo(() => groups.flat(), [groups]);

  /** Flat index of a group's first row, so cursor state survives grouping. */
  const offsetOf = (groupIndex: number) =>
    groups.slice(0, groupIndex).reduce((n, g) => n + g.length, 0);

  useEffect(() => {
    if (open) setCursor(0);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setOpen(false);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        setCursor((c) => Math.min(c + 1, actions.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        setCursor((c) => Math.max(c - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        const action = actions[cursor];
        if (action) {
          setOpen(false);
          action.run();
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, actions, cursor, setOpen]);

  if (!open) return null;

  return (
    <div
      className="animate-fade absolute inset-0 z-40 flex items-start justify-center bg-black/8 pt-16 dark:bg-black/40"
      onMouseDown={() => setOpen(false)}
    >
      <div
        role="menu"
        // `shadow-menu` is one step below `shadow-panel`: this floats over the
        // window, it is not the window.
        className="animate-pop w-84 overflow-hidden rounded-xl bg-panel p-2 shadow-menu ring-1 ring-panel-ring"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Section label: the lowest tier of the type scale, and the faintest
            tone that still passes AA. It names the rows below it, it does not
            compete with them. */}
        <div className="px-2 py-2 text-micro font-semibold text-faint">
          Actions
        </div>

        {groups.map((group, gi) => {
          const offset = offsetOf(gi);
          return (
            <Fragment key={gi}>
              {gi > 0 && (
                <div aria-hidden className="mx-2 my-1 h-px bg-divider" />
              )}
              {group.map((action, i) => {
                const cursorIndex = offset + i;
                const Icon = action.icon;
                return (
                  <button
                    key={action.id}
                    type="button"
                    role="menuitem"
                    onMouseEnter={() => setCursor(cursorIndex)}
                    onClick={() => {
                      setOpen(false);
                      action.run();
                    }}
                    // Same 32px row height as the TypeFilter options: two
                    // menus, one row.
                    className={cn(
                      "flex h-8 w-full items-center gap-3 rounded-lg px-2 text-left text-body transition-colors",
                      cursorIndex === cursor && "bg-selected",
                      action.danger ? "text-danger" : "text-ink",
                    )}
                  >
                    <Icon
                      className={cn(
                        "h-4 w-4 shrink-0",
                        action.danger ? "text-danger" : "text-ink/70",
                      )}
                      strokeWidth={1.75}
                    />
                    <span className="truncate">{action.label}</span>
                  </button>
                );
              })}
            </Fragment>
          );
        })}
      </div>
    </div>
  );
}
