import type { ReactNode } from "react";
import type { ClipDetail, ClipKind } from "../../types/clip";
import { formatBytes, formatCopied, formatDimensions } from "../../lib/format";
import { AppIcon } from "../common/AppIcon";

const CONTENT_TYPE: Record<ClipKind, string> = {
  text: "Text",
  image: "Image",
  link: "Link",
  color: "Color",
  file: "File",
};

/**
 * One metadata line. Every row except the last carries a hairline under it,
 * and the rules run the full width of the pane: the 16px gutter lives on the
 * row itself, not on the section wrapper.
 */
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex h-7 items-center justify-between gap-4 border-b border-divider px-4 last:border-b-0">
      <span className="shrink-0 text-[13px] text-muted">{label}</span>
      <span className="flex min-w-0 items-center gap-1.5 text-[13px] text-ink">
        {children}
      </span>
    </div>
  );
}

/** Metadata block under the preview: the `Information` section. */
export function InfoSection({ clip }: { clip: ClipDetail }) {
  const isText = clip.kind === "text";

  return (
    <div className="flex flex-col">
      {/* Section heading: same tone and size as the labels below, 28px tall,
          and the only row with no rule under it. */}
      <div className="flex h-7 items-center px-4 text-[13px] text-muted">
        Information
      </div>

      <Row label="Source">
        <AppIcon name={clip.sourceApp} src={clip.sourceIcon} />
        <span className="truncate">{clip.sourceApp ?? "Unknown"}</span>
      </Row>

      <Row label="Content type">{CONTENT_TYPE[clip.kind]}</Row>

      {clip.kind === "image" && (
        <>
          <Row label="Dimensions">
            {formatDimensions(clip.width, clip.height)}
          </Row>
          <Row label="Image size">{formatBytes(clip.byteSize)}</Row>
        </>
      )}

      {isText && (
        <>
          <Row label="Characters">{clip.characters ?? "—"}</Row>
          <Row label="Words">{clip.words ?? "—"}</Row>
        </>
      )}

      <Row label="Copied">{formatCopied(clip.createdAt)}</Row>
    </div>
  );
}
