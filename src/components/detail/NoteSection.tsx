import { useEffect, useRef, useState } from "react";
import { Pencil } from "lucide-react";
import { useApp } from "../../stores/useApp";
import { cn } from "../../lib/cn";
import { Kbd } from "../common/Kbd";

/**
 * Maximum note length. Long enough for a URL plus a sentence of context,
 * short enough that the block never pushes `Information` off the panel.
 */
const MAX = 500;

/** The counter only appears once the note is within reach of the ceiling. */
const COUNTER_FROM = 400;

/**
 * The user-authored note attached to one entry.
 *
 * Sits between the preview and `Information` rather than inside either: it is
 * metadata about the capture, but unlike the read-only rows below it, it is
 * editable in place.
 *
 * Three states, all driven by `description === null`:
 *   empty  - a dashed affordance that reads as a prompt, not as a field
 *   filled - the text, with the edit hint revealed on hover
 *   editing- a textarea, focus-trapped until Return or Escape
 *
 * Saving happens on blur as well as `Cmd+Return`, because a note edited and
 * then abandoned by clicking the list should not silently discard itself the
 * way it would in a form.
 */
export function NoteSection() {
  const selectedId = useApp((s) => s.selectedId);
  const items = useApp((s) => s.items);
  const setDescription = useApp((s) => s.setDescription);
  const noteEditingId = useApp((s) => s.noteEditingId);
  const beginNoteEdit = useApp((s) => s.beginNoteEdit);
  const endNoteEdit = useApp((s) => s.endNoteEdit);

  const item = items.find((i) => i.id === selectedId) ?? null;
  const saved = item?.description ?? null;

  const editing = noteEditingId != null && noteEditingId === selectedId;
  const [draft, setDraft] = useState("");
  const areaRef = useRef<HTMLTextAreaElement | null>(null);

  // Switching rows abandons an open editor: the draft belonged to the entry
  // that was selected when the user started typing.
  useEffect(() => {
    if (noteEditingId != null && noteEditingId !== selectedId) endNoteEdit();
  }, [noteEditingId, selectedId, endNoteEdit]);

  useEffect(() => {
    if (!editing) return;
    const area = areaRef.current;
    if (!area) return;
    area.focus();
    area.setSelectionRange(area.value.length, area.value.length);
  }, [editing]);

  if (!item) return null;

  const commit = async (value: string) => {
    endNoteEdit();
    const trimmed = value.trim();
    if (trimmed === (saved ?? "")) return;
    await setDescription(item.id, trimmed === "" ? null : trimmed);
  };

  const startEditing = () => {
    setDraft(saved ?? "");
    beginNoteEdit();
  };

  if (editing) {
    return (
      <div className="shrink-0 px-4 pb-2.5">
        {/* Grey, not blue. This panel has no "coloured focus ring" language:
            hover is `bg-hover`, selection is `bg-selected`, and an open
            control (see TypeFilter) is `border-line` over `bg-hover`. A
            3px accent glow was the only chromatic box-shadow in the app, and
            it read louder than anything else on screen. The caret is what
            marks focus here, exactly as it does in the two search inputs. */}
        <div className="overflow-hidden rounded-lg border border-line bg-hover">
          <textarea
            ref={areaRef}
            value={draft}
            maxLength={MAX}
            spellCheck={false}
            placeholder="为什么留着这条？"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              // Return alone inserts a newline: the note is multi-line and
              // Return is the natural way to break one. Save is Cmd+Return,
              // which the global nav would otherwise read as "paste".
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                e.stopPropagation();
                void commit(draft);
                return;
              }
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                endNoteEdit();
                return;
              }
              // Everything else is the global nav's business, but it must
              // not see a bare Return on its way to Cmd+Return above.
              if (e.key === "Enter") e.stopPropagation();
            }}
            onBlur={() => void commit(draft)}
            className="block min-h-15.5 w-full resize-none border-0 bg-app px-2.5 py-1.5 text-[13px] leading-[1.5] text-ink outline-none"
          />
          {/* The container is `bg-hover` too, so the bar needs its own rule to
              read as a separate strip — same device the Information rows use
              (`border-b border-divider` between label/value lines). */}
          <div className="flex items-center justify-between border-t border-divider bg-hover px-2.5 py-1">
            <span className="flex items-center gap-1 text-[11.5px] text-faint">
              <Kbd>⌘</Kbd>
              <Kbd>↵</Kbd>
              save
              <span className="mx-0.5 text-line">·</span>
              <Kbd>esc</Kbd>
              cancel
            </span>
            {draft.length >= COUNTER_FROM && (
              <span
                className={cn(
                  "font-mono text-[11.5px] text-faint",
                  draft.length >= MAX && "text-accent",
                )}
              >
                {draft.length} / {MAX}
              </span>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (saved === null) {
    return (
      <div className="shrink-0 px-4 pb-2.5">
        <button
          type="button"
          onClick={startEditing}
          className={cn(
            "flex h-7.5 w-full items-center gap-1.5 rounded-lg border border-dashed border-line",
            "px-2.5 text-[13px] text-faint transition-colors duration-75",
            "hover:border-faint hover:bg-hover hover:text-muted",
          )}
        >
          <Pencil className="h-3 w-3 shrink-0" strokeWidth={1.7} />
          Add a note
        </button>
      </div>
    );
  }

  return (
    <div className="group/note shrink-0 px-4 pb-2.5">
      <div
        role="group"
        tabIndex={0}
        onClick={startEditing}
        onKeyDown={(e) => {
          // The button equivalent is what the keyboard contract already
          // promises elsewhere: Return activates a focused control.
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            startEditing();
          }
        }}
        className={cn(
          "relative min-h-7.5 cursor-default rounded-lg border border-transparent",
          "px-2.5 py-1.5 text-[13px] leading-[1.5] break-words text-ink",
          "hover:border-divider hover:bg-hover",
        )}
      >
        {saved}
        <Pencil
          className="pointer-events-none absolute top-2 right-2 h-3 w-3 text-faint opacity-0 transition-opacity duration-75 group-hover/note:opacity-100"
          strokeWidth={1.7}
        />
      </div>
    </div>
  );
}
