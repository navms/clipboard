import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "../lib/cn";
import { useApp } from "../stores/useApp";
import type { TypeFilter as TypeFilterValue } from "../types/clip";

const OPTIONS: { value: TypeFilterValue; label: string }[] = [
  { value: "all", label: "All Types" },
  { value: "text", label: "Texts Only" },
  { value: "images", label: "Images Only" },
  { value: "files", label: "Files Only" },
  { value: "links", label: "Links Only" },
  { value: "colors", label: "Colors Only" },
];

export function TypeFilter() {
  const filter = useApp((s) => s.filter);
  const setFilter = useApp((s) => s.setFilter);
  const open = useApp((s) => s.typeFilterOpen);
  const setOpen = useApp((s) => s.setTypeFilterOpen);

  const [search, setSearch] = useState("");
  const [cursor, setCursor] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const current = OPTIONS.find((o) => o.value === filter) ?? OPTIONS[0];

  const visible = useMemo(() => {
    const q = search.trim().toLowerCase();
    return q ? OPTIONS.filter((o) => o.label.toLowerCase().includes(q)) : OPTIONS;
  }, [search]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, setOpen]);

  useEffect(() => {
    if (open) {
      setSearch("");
      setCursor(OPTIONS.findIndex((o) => o.value === filter));
      requestAnimationFrame(() => searchRef.current?.focus());
    }
  }, [open, filter]);

  const commit = (value: TypeFilterValue) => {
    setFilter(value);
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className={cn(
          "flex h-8 w-46.5 items-center justify-between rounded-[9px] border px-2.5",
          "text-[13px] text-ink transition-colors",
          open
            ? "border-line bg-hover"
            : "border-line bg-transparent hover:bg-hover",
        )}
      >
        <span className="truncate">{current.label}</span>
        <ChevronDown
          className={cn(
            "h-3.5 w-3.5 shrink-0 text-muted transition-transform duration-150",
            open && "rotate-180",
          )}
          strokeWidth={2.25}
        />
      </button>

      {open && (
        <div
          role="listbox"
          className="animate-pop absolute right-0 top-[calc(100%_+_6px)] z-30 w-46.5 rounded-xl bg-panel p-2 shadow-menu"
        >
          <input
            ref={searchRef}
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setCursor(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setCursor((c) => Math.min(c + 1, visible.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setCursor((c) => Math.max(c - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                const pick = visible[cursor];
                if (pick) commit(pick.value);
              }
            }}
            placeholder="Search..."
            spellCheck={false}
            className="mb-1.5 h-7.5 w-full rounded-lg border border-line bg-transparent px-2.5 text-[13px] text-ink outline-none placeholder:text-faint"
          />

          <div className="flex flex-col">
            {visible.map((option, i) => {
              const selected = option.value === filter;
              return (
                <button
                  key={option.value}
                  type="button"
                  role="option"
                  aria-selected={selected}
                  onMouseEnter={() => setCursor(i)}
                  onClick={() => commit(option.value)}
                  className={cn(
                    "flex h-8 items-center rounded-lg px-2.5 text-left text-[13px] text-ink",
                    "transition-colors",
                    selected
                      ? "bg-selected"
                      : i === cursor
                        ? "bg-hover"
                        : "bg-transparent",
                  )}
                >
                  {option.label}
                </button>
              );
            })}
            {visible.length === 0 && (
              <div className="px-2.5 py-2 text-[13px] text-muted">
                No matching types
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
