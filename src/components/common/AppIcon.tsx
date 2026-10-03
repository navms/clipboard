import { cn } from "../../lib/cn";

const BRAND: Record<string, string> = {
  "Google Chrome": "#4285F4",
  Safari: "#0FB5EE",
  PixPin: "#FF6B57",
  Figma: "#A259FF",
  Notes: "#FFC300",
  Finder: "#1E8FFD",
  "CleanShot X": "#7B61FF",
  Xcode: "#1575F9",
  Terminal: "#4B4B4F",
  Code: "#2C9AD6",
  Slack: "#611F69",
};

const FALLBACK = [
  "#5B8DEF",
  "#E8654F",
  "#8E5BEF",
  "#E8A33D",
  "#3FB68B",
  "#D96BA0",
];

function hashColor(name: string): string {
  let h = 0;
  for (let i = 0; i < name.length; i += 1) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return FALLBACK[h % FALLBACK.length];
}

/**
 * Stand-in for a macOS app icon: a rounded brand-coloured tile with the
 * app's initial. Real icons can later come from
 * `NSWorkspace.icon(forFile:)` on the Rust side without touching callers.
 */
export function AppIcon({
  name,
  className,
}: {
  name?: string | null;
  className?: string;
}) {
  if (!name) {
    return (
      <span
        className={cn(
          "h-4 w-4 shrink-0 rounded-sm bg-black/[0.07] dark:bg-white/12",
          className,
        )}
      />
    );
  }

  return (
    <span
      className={cn(
        "flex h-4 w-4 shrink-0 items-center justify-center rounded-sm text-[9px] font-semibold leading-none text-white",
        className,
      )}
      style={{ backgroundColor: BRAND[name] ?? hashColor(name) }}
      title={name}
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
