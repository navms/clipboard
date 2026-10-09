import { memo } from "react";
import { Pencil, Pin } from "lucide-react";
import { cn } from "../lib/cn";
import type { ClipListItem } from "../types/clip";
import { ClipIcon } from "./common/ClipIcon";

interface Props {
  item: ClipListItem;
  selected: boolean;
  onSelect: (id: number) => void;
  onPaste: () => void;
}

export const EntryRow = memo(function EntryRow({
  item,
  selected,
  onSelect,
  onPaste,
}: Props) {
  return (
    <button
      type="button"
      onClick={() => onSelect(item.id)}
      onDoubleClick={onPaste}
      className={cn(
        "group flex h-10 w-full items-center gap-3 rounded-lg px-2 text-left",
        "transition-colors duration-75",
        selected ? "bg-selected" : "hover:bg-hover",
      )}
    >
      <ClipIcon
        kind={item.kind}
        thumbPath={item.thumbPath}
        color={item.color}
      />
      <span className="min-w-0 flex-1 truncate text-body leading-none text-ink">
        {item.title}
      </span>
      {/* A fixed 14px slot, occupied whether or not a note exists, so the
          title truncates at the same point on every row. Rendering the note
          text itself would have to grow the row, and `ROW_H` in EntryList is
          a constant the virtualiser measures against. */}
      {item.description ? (
        <Pencil
          className="h-3.5 w-3.5 shrink-0 text-faint"
          strokeWidth={1.7}
        />
      ) : (
        <span className="h-3.5 w-3.5 shrink-0" />
      )}
      {item.pinned && (
        <Pin className="h-3.5 w-3.5 shrink-0 text-faint" strokeWidth={2} />
      )}
    </button>
  );
});
