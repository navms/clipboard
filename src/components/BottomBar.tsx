import { TriangleAlert, X } from "lucide-react";
import { useApp } from "../stores/useApp";
import { AppGlyph } from "./common/AppGlyph";
import { Kbd } from "./common/Kbd";

export function BottomBar() {
  const targetApp = useApp((s) => s.targetApp);
  const hasSelection = useApp((s) => s.selectedId != null);
  const pasteSelected = useApp((s) => s.pasteSelected);
  const setActionsOpen = useApp((s) => s.setActionsOpen);
  const pasteBlocked = useApp((s) => s.pasteBlocked);
  const dismissPasteBlocked = useApp((s) => s.dismissPasteBlocked);
  const openAccessibilitySettings = useApp((s) => s.openAccessibilitySettings);

  return (
    <footer className="flex h-10.5 shrink-0 items-center justify-between gap-3 border-t border-divider bg-app px-3">
      {pasteBlocked ? (
        <PermissionHint
          onOpen={() => void openAccessibilitySettings()}
          onDismiss={dismissPasteBlocked}
        />
      ) : (
        <>
          {/* Negative margin + matching padding lets the hover pill extend
              past the row without nudging the resting position of the icon. */}
          <div className="-mx-1.5 flex min-w-0 items-center gap-3 rounded-md px-1.5 py-1 transition-colors hover:bg-hover">
            <AppGlyph />
            <span className="truncate text-[13px] text-muted">
              Clipboard History
            </span>
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              disabled={!hasSelection}
              onClick={() => void pasteSelected()}
              className="flex items-center gap-2.5 rounded-[7px] px-2 py-1 text-[13px] font-semibold text-ink transition-colors hover:bg-hover disabled:cursor-default disabled:opacity-45 disabled:hover:bg-transparent"
            >
              <span className="truncate">
                Paste to {targetApp ?? "the active app"}
              </span>
              <Kbd>↵</Kbd>
            </button>

            {/* Hairline between the primary and secondary shortcut groups. It
                uses the same tone as the keycaps so the two groups read as
                one set. */}
            <span aria-hidden className="h-2.75 w-px shrink-0 bg-keycap" />

            <button
              type="button"
              onClick={() => setActionsOpen(true)}
              className="flex items-center gap-2.5 rounded-[7px] py-1 pl-2 text-[13px] text-muted transition-colors hover:bg-hover"
            >
              <span>Actions</span>
              <span className="flex items-center gap-0.5">
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </span>
            </button>
          </div>
        </>
      )}
    </footer>
  );
}

/**
 * Takes over the whole footer when a paste was refused.
 *
 * It replaces the normal content rather than stacking above it, so the panel
 * height never moves: this appears exactly when the user expected the window
 * to disappear, and a jumping layout would make that worse. The clip is on
 * the pasteboard regardless, hence the reassurance on the right of the label.
 */
function PermissionHint({
  onOpen,
  onDismiss,
}: {
  onOpen: () => void;
  onDismiss: () => void;
}) {
  return (
    <>
      <div className="flex min-w-0 items-center gap-2">
        <TriangleAlert
          className="h-4 w-4 shrink-0 text-[#e5484d]"
          strokeWidth={1.75}
        />
        <span className="truncate text-[13px] text-ink">
          Allow Accessibility to paste automatically
        </span>
        <span className="truncate text-[13px] text-faint">
          · copied to clipboard
        </span>
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <button
          type="button"
          onClick={onOpen}
          className="rounded-[7px] px-2 py-1 text-[13px] font-semibold text-accent transition-colors hover:bg-hover"
        >
          Open System Settings
        </button>
        <button
          type="button"
          aria-label="Dismiss"
          onClick={onDismiss}
          className="flex h-6 w-6 items-center justify-center rounded-[7px] text-muted transition-colors hover:bg-hover"
        >
          <X className="h-3.5 w-3.5" strokeWidth={2} />
        </button>
      </div>
    </>
  );
}
