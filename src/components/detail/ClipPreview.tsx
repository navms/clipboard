import {
  ExternalLink,
  File as FileIcon,
  FileArchive,
  FileCode,
  FileText,
  Folder,
  Image as ImageIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { useApp } from "../../stores/useApp";
import { cn } from "../../lib/cn";
import { Kbd } from "../common/Kbd";
import type { ClipDetail } from "../../types/clip";

const IMAGE_EXT = new Set([
  "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "tiff", "tif", "bmp",
  "svg", "avif", "ico",
]);
const CODE_EXT = new Set([
  "ts", "tsx", "js", "jsx", "mjs", "cjs", "rs", "py", "go", "java", "kt",
  "swift", "c", "cc", "cpp", "h", "hpp", "rb", "php", "sh", "zsh", "bash",
  "css", "scss", "sass", "html", "vue", "svelte", "sql",
]);
const DOC_EXT = new Set([
  "md", "txt", "rtf", "pdf", "doc", "docx", "pages", "xls", "xlsx", "numbers",
  "ppt", "pptx", "key", "csv",
]);
const ARCHIVE_EXT = new Set([
  "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "dmg", "pkg",
]);

/**
 * Picks a glyph from the file name's extension.
 *
 * A name with no extension is guessed to be a *folder*: these paths come off
 * the pasteboard, where copying a folder is far more common than copying an
 * extension-less file. The guess is cosmetic and wrong only for the odd
 * `Makefile`, not worth a filesystem stat, which would also fail outright for
 * something that has since been moved or deleted.
 */
function glyphFor(name: string) {
  const dot = name.lastIndexOf(".");
  // A leading dot marks a hidden file (".zshrc"), not an extension.
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  if (!ext) return Folder;
  if (IMAGE_EXT.has(ext)) return ImageIcon;
  if (CODE_EXT.has(ext)) return FileCode;
  if (DOC_EXT.has(ext)) return FileText;
  if (ARCHIVE_EXT.has(ext)) return FileArchive;
  return FileIcon;
}

/**
 * Splits a path into what it is and where it lives.
 *
 * `/Users/me/docs/report.pdf` reads far better as "report.pdf" over
 * "/Users/me/docs" than as one long monospaced line, and the pane is 380px
 * wide, so an unsplit path wraps mid-word and becomes a wall.
 */
function splitPath(path: string): { name: string; dir: string | null } {
  const cut = path.lastIndexOf("/");
  if (cut === -1) return { name: path, dir: null };
  return { name: path.slice(cut + 1) || path, dir: path.slice(0, cut) || "/" };
}

/**
 * Splits a URL so the host can carry the weight and the rest can recede.
 *
 * Returns `null` when `URL` refuses to parse; the caller then shows the raw
 * string, which is honester than a half-parsed guess.
 */
function splitUrl(raw: string): { host: string; rest: string } | null {
  try {
    const url = new URL(raw);
    const rest = `${url.pathname}${url.search}${url.hash}`;
    return {
      host: url.host.replace(/^www\./, ""),
      // A bare origin parses to pathname "/", which is noise in a preview.
      rest: rest === "/" ? "" : rest,
    };
  } catch {
    return null;
  }
}

/**
 * One clickable content row: glyph, a label line, an optional detail line, and
 * a hover pill.
 *
 * The pill is negative-margined into the pane's 16px gutter, so the row's
 * *text* keeps the normal inset while the hover surface reaches past it,
 * the same trick the bottom bar uses around its app glyph.
 */
function ContentRow({
  icon: Icon,
  iconClass,
  label,
  detail,
  title,
  onClick,
}: {
  icon: typeof ExternalLink;
  iconClass?: string;
  label: ReactNode;
  detail?: ReactNode;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className="group -mx-1.5 flex items-start gap-2.5 rounded-[7px] px-1.5 py-1.5 text-left transition-colors hover:bg-hover"
    >
      <Icon
        className={cn("mt-0.5 h-4 w-4 shrink-0", iconClass)}
        strokeWidth={1.75}
      />
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="text-[13px] leading-snug wrap-break-word">
          {label}
        </span>
        {detail != null && (
          <span className="line-clamp-2 font-mono text-[11px] leading-snug text-muted wrap-break-word">
            {detail}
          </span>
        )}
      </span>
    </button>
  );
}

/**
 * The Cmd+O hint under a row. Its leading spacer is the glyph's width, so the
 * hint sits under the label rather than under the icon.
 */
function ActionHint({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 text-[11px] text-faint">
      <span aria-hidden className="h-4 w-4 shrink-0" />
      <span className="flex items-center gap-1.5">
        <span className="flex items-center gap-0.5">
          <Kbd className="h-4.5 min-w-5 rounded-[4px] text-[11px]">⌘</Kbd>
          <Kbd className="h-4.5 min-w-5 rounded-[4px] text-[11px]">O</Kbd>
        </span>
        {children}
      </span>
    </div>
  );
}

/** The upper half of the detail pane: a per-kind rendering of the content. */
export function ClipPreview({ clip }: { clip: ClipDetail }) {
  const openSelectedLink = useApp((s) => s.openSelectedLink);
  const revealSelectedFiles = useApp((s) => s.revealSelectedFiles);

  switch (clip.kind) {
    case "image":
      return (
        <div className="flex h-full items-start justify-center">
          {clip.imagePath || clip.thumbPath ? (
            <img
              src={clip.imagePath ?? clip.thumbPath ?? ""}
              alt={clip.title}
              draggable={false}
              className="max-h-full w-auto max-w-full rounded-[10px] object-contain ring-1 ring-black/10 dark:ring-white/10"
            />
          ) : (
            <div className="text-[13px] text-faint">Image unavailable</div>
          )}
        </div>
      );

    case "color": {
      // The pasteboard may hold `#abc`, `#aabbcc`, `rgb(...)` or `hsl(...)`, so
      // the halo is derived with `color-mix()` rather than parsed in JS: it
      // accepts every CSS colour syntax. Inline styles only.
      const swatch = (clip.color ?? clip.contentText ?? "").trim() || "#000000";
      return (
        // `inset-0` pins the layer to the pane's *padding* box, so the swatch
        // centres on the visible pane instead of the padded content box that
        // the wrapper's `pt-3.5` carves out. (An earlier `calc(100% + N)`
        // trick silently failed: CSS requires spaces around `+` in `calc()`.)
        <div className="absolute inset-0 flex items-center justify-center">
          <div
            className="size-20 rounded-full"
            style={{
              backgroundColor: swatch,
              boxShadow: `0 0 0 5px color-mix(in srgb, ${swatch} 42%, transparent)`,
            }}
          />
          <span className="absolute left-1/2 top-[calc(50%+58px)] -translate-x-1/2 text-[13px] text-ink">
            {swatch}
          </span>
        </div>
      );
    }

    case "file": {
      // The payload is one absolute path per line; the watcher joins them.
      const paths = (clip.contentText ?? "")
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean);

      if (!paths.length) {
        return <div className="text-[13px] text-muted">{clip.title}</div>;
      }

      return (
        // `-mt-1.5` cancels the first row's own padding so the text lands where
        // every other kind's content starts, while the hover pill keeps its
        // full surface.
        <div className="-mt-1.5 flex flex-col gap-0.5">
          {paths.map((path, index) => {
            const { name, dir } = splitPath(path);
            return (
              <ContentRow
                key={`${index}:${path}`}
                icon={glyphFor(name)}
                iconClass="text-muted group-hover:text-ink"
                label={<span className="font-medium text-ink">{name}</span>}
                detail={dir}
                title={path}
                onClick={() => void revealSelectedFiles()}
              />
            );
          })}

          <ActionHint>
            Reveal in Finder
            {paths.length > 1 ? ` · ${paths.length} items` : ""}
          </ActionHint>
        </div>
      );
    }

    case "link": {
      const raw = (clip.contentText ?? clip.title).trim();
      const parts = splitUrl(raw);

      return (
        <div className="-mt-1.5 flex flex-col gap-0.5">
          {/* The URL *is* the button: a menu entry that appears for only one
              kind makes the user remember where it went, and the thing being
              opened is a better target than a label in a list.

              Nothing here is tinted. An all-blue paragraph reads as a
              hyperlink pasted into a page rather than as content, and a blue
              glyph alongside it is the same signal twice: the link glyph, the
              Cmd+O hint and the hover pill already say "you can act on this".
              Colouring it like a file row also stops the pane from changing
              character depending on what happens to be selected. The host
              keeps the weight, so a long URL stays scannable without shouting. */}
          <ContentRow
            icon={ExternalLink}
            iconClass="text-muted group-hover:text-ink"
            label={
              parts ? (
                <>
                  <span className="font-medium text-ink group-hover:underline">
                    {parts.host}
                  </span>
                  {parts.rest && (
                    <span className="text-muted group-hover:underline">
                      {parts.rest}
                    </span>
                  )}
                </>
              ) : (
                <span className="text-ink group-hover:underline">{raw}</span>
              )
            }
            title={raw}
            onClick={() => void openSelectedLink()}
          />

          <ActionHint>Open in browser</ActionHint>
        </div>
      );
    }

    case "text":
    default:
      return (
        <div className="whitespace-pre-wrap wrap-break-word text-[13px] leading-relaxed text-ink">
          {clip.contentText ?? clip.title}
        </div>
      );
  }
}
