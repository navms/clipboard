import { cn } from "../../lib/cn";

/**
 * The app's own glyph: an *outlined* clipboard on a warm gradient tile.
 *
 * The outline is a stroked rounded rect plus a small tab drawn on top of it.
 * The tab carries the tile gradient as its fill so it punches out the body's
 * top edge and reads as a body with a bump.
 * `gradientUnits="userSpaceOnUse"` keeps that fill colour continuous with the
 * tile behind it.
 */
export function AppGlyph({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-4.5 w-4.5 shrink-0 items-center justify-center overflow-hidden",
        className,
      )}
      aria-hidden
    >
      <svg viewBox="0 0 16 16" className="h-full w-full">
        <defs>
          <linearGradient
            id="rc-glyph"
            x1="0"
            y1="0"
            x2="16"
            y2="16"
            gradientUnits="userSpaceOnUse"
          >
            <stop offset="0%" stopColor="#ff6b57" />
            <stop offset="100%" stopColor="#e8402a" />
          </linearGradient>
        </defs>
        <rect width="16" height="16" rx="4.6" fill="url(#rc-glyph)" />
        <rect
          x="4.5"
          y="4.1"
          width="7.1"
          height="8.9"
          rx="1.9"
          fill="none"
          stroke="#fff"
          strokeWidth="1.26"
        />
        <rect
          x="6.3"
          y="2.5"
          width="3.4"
          height="2.7"
          rx="1.1"
          fill="url(#rc-glyph)"
          stroke="#fff"
          strokeWidth="1.26"
        />
      </svg>
    </span>
  );
}
