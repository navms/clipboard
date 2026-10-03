import { File, FileText, ImageIcon, Link2 } from "lucide-react";
import { cn } from "../../lib/cn";
import type { ClipKind } from "../../types/clip";

interface Props {
  kind: ClipKind;
  /** Thumbnail (data-URI in preview, asset URL in Tauri). */
  thumbPath?: string | null;
  /** Swatch fill for `kind: "color"`. */
  color?: string | null;
  className?: string;
}

/**
 * The 20x20 leading glyph for a history row. Images get their real
 * thumbnail; colours get a filled disc; everything else a muted outline.
 */
export function ClipIcon({ kind, thumbPath, color, className }: Props) {
  const box = cn("flex h-5 w-5 shrink-0 items-center justify-center", className);

  if (kind === "image" && thumbPath) {
    return (
      <span className={box}>
        <img
          src={thumbPath}
          alt=""
          draggable={false}
          className="h-5 w-5 rounded-sm object-cover ring-1 ring-black/10 dark:ring-white/10"
        />
      </span>
    );
  }

  if (kind === "color" && color) {
    return (
      <span className={box}>
        <span
          className="h-3.5 w-3.5 rounded-full ring-2 ring-black/20 dark:ring-white/25"
          style={{ backgroundColor: color }}
        />
      </span>
    );
  }

  const glyph = "h-5 w-5 text-ink/70";

  switch (kind) {
    case "image":
      return (
        <span className={box}>
          <ImageIcon className={glyph} strokeWidth={1.75} />
        </span>
      );
    case "link":
      return (
        <span className={box}>
          <Link2 className={glyph} strokeWidth={1.75} />
        </span>
      );
    case "file":
      return (
        <span className={box}>
          <File className={glyph} strokeWidth={1.75} />
        </span>
      );
    case "text":
    default:
      return (
        <span className={box}>
          <FileText className={glyph} strokeWidth={1.75} />
        </span>
      );
  }
}
