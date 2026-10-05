import { useState } from "react";
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
 * An application's icon, falling back to a brand-coloured initial.
 *
 * The real thing is resolved on the Rust side from the bundle id and arrives
 * as an asset URL; the tile is what the caller sees when it could not be had,
 * which is most often because the source app is not installed any more or
 * never declared a bundle id. Both are ordinary states rather than errors, and
 * they look the same here on purpose — a row that says "Unknown" is a bug
 * report waiting to happen, and an initial still identifies the app.
 *
 * `onError` is not paranoid: the path is resolved on read and cached forever,
 * so a user who empties the data dir or uninstalls the app after first
 * viewing an entry gets a path that no longer resolves. The `<img>` would
 * then render its broken-image glyph inside a 16px box, which reads as noise
 * rather than as a fallback.
 */
export function AppIcon({
  name,
  src,
  className,
}: {
  name?: string | null;
  /** Asset URL of the real icon, when one was resolved. */
  src?: string | null;
  className?: string;
}) {
  const [broken, setBroken] = useState(false);

  if (src && !broken) {
    return (
      <img
        src={src}
        alt=""
        title={name ?? undefined}
        draggable={false}
        onError={() => setBroken(true)}
        // Not a `ring`. A ring is a box-shadow on the border box, so it only
        // follows `border-radius` — and this element has none, because macOS
        // already drew the rounded square into the bitmap's alpha. A ring with
        // no radius to follow comes out as a plain square, and the four corners
        // land on transparent pixels: the icon ends up wearing a rectangle.
        //
        // `drop-shadow` filters the alpha channel instead, so the shadow
        // traces the icon's own silhouette. The result reads as the edge macOS
        // gives every app icon, rather than a frame drawn around it.
        className={cn(
          "h-4 w-4 shrink-0 object-cover drop-shadow-[0_0_1px_rgba(0,0,0,0.28)]",
          className,
        )}
      />
    );
  }

  if (!name) {
    return (
      <span
        className={cn(
          "h-4 w-4 shrink-0 rounded-sm bg-black/8 dark:bg-white/12",
          className,
        )}
      />
    );
  }

  return (
    <span
      className={cn(
        "flex h-4 w-4 shrink-0 items-center justify-center rounded-sm text-micro font-semibold leading-none text-white",
        className,
      )}
      style={{ backgroundColor: BRAND[name] ?? hashColor(name) }}
      title={name}
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
