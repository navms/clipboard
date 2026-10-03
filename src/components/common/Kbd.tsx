import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

/**
 * Small keycap badge: the Return / Cmd / K chips in the bottom bar.
 *
 * These are *solid* chips with no outline, so we fill with the chip tone and
 * carry no border at all. Measured at 24x21 CSS with a ~5px radius, glyph
 * muted. `min-w` keeps one-glyph chips square-ish; wider glyphs simply grow.
 *
 * Multi-glyph shortcuts are rendered as one `<Kbd>` per key: Cmd and K are
 * two separate chips, not a single "Cmd+K" badge.
 */
export function Kbd({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <kbd
      className={cn(
        "inline-flex h-5.25 min-w-6 items-center justify-center rounded-[5px]",
        "bg-keycap px-1",
        "font-sans text-[12px] font-medium leading-none text-muted",
        className,
      )}
    >
      {children}
    </kbd>
  );
}
