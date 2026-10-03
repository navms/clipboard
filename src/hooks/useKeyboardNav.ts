import { useEffect } from "react";
import { useApp } from "../stores/useApp";
import * as ipc from "../lib/ipc";

/**
 * Global shortcuts for the clipboard panel:
 * up/down navigate, Return paste, Cmd+K actions, Cmd+O open/reveal,
 * Cmd+P pin, Cmd+D note, Cmd+Delete delete, Cmd+1/Cmd+2 rail tabs, Esc dismiss.
 */
export function useKeyboardNav() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useApp.getState();
      const meta = e.metaKey || e.ctrlKey;
      const key = e.key;

      // The Cmd+K menu owns the keyboard while it is open.
      if (s.actionsOpen) return;

      // The note editor owns the keyboard while it is open. It is listed
      // here rather than relying on the textarea to stop every event,
      // because the ones that matter — a bare Return, Escape — are exactly
      // the ones the block below is written to claim.
      if (s.noteEditingId != null) {
        if (key === "Escape") {
          e.preventDefault();
          s.endNoteEdit();
        } else if (key === "Enter" && meta) {
          e.preventDefault();
          // The textarea handles the commit itself on this key; swallowing
          // it here is what keeps it from also reaching the paste branch.
        }
        return;
      }

      // The type dropdown owns up/down/Return while it is open.
      if (s.typeFilterOpen) {
        if (key === "Escape") s.setTypeFilterOpen(false);
        return;
      }

      if (meta && key.toLowerCase() === "k") {
        e.preventDefault();
        s.setActionsOpen(true);
        return;
      }
      if (meta && key.toLowerCase() === "o") {
        // Cmd+O is macOS's "Open", claimed globally: this window is app chrome
        // rather than a document, so there is no browser Cmd+O worth preserving.
        // `openSelected` maps it per kind (link opens, file reveals) and
        // falls through silently for the kinds without a primary action.
        e.preventDefault();
        void s.openSelected();
        return;
      }
      if (meta && key.toLowerCase() === "p") {
        e.preventDefault();
        if (s.selectedId != null) void s.togglePin(s.selectedId);
        return;
      }
      if (meta && key.toLowerCase() === "d") {
        // Cmd+D edits the note on the selected entry. Claimed before the
        // bare-Return branch below, which has no meta guard of its own and
        // would otherwise read a Cmd+Return from the note editor as "paste".
        e.preventDefault();
        s.beginNoteEdit();
        return;
      }
      if (meta && key === "Backspace") {
        e.preventDefault();
        if (s.selectedId != null) void s.remove(s.selectedId);
        return;
      }

      // Cmd+1 / Cmd+2 switch rail tabs. Numbers rather than letters because
      // Cmd+P already means "pin", and Cmd+H/I/etc. are macOS-wide. These are
      // the browser's tab-numbering keys, but this window is app chrome with
      // exactly one document in it, so there is nothing else for them to mean.
      // Without them the shelf is mouse-only inside a keyboard-first panel.
      if (meta && (key === "1" || key === "2")) {
        e.preventDefault();
        s.setView(key === "1" ? "history" : "pinned");
        return;
      }

      if (key === "ArrowDown") {
        e.preventDefault();
        s.moveSelection(1);
        return;
      }
      if (key === "ArrowUp") {
        e.preventDefault();
        s.moveSelection(-1);
        return;
      }
      if (key === "Enter") {
        e.preventDefault();
        void s.pasteSelected();
        return;
      }
      if (key === "Escape") {
        if (s.query) {
          s.setQuery("");
          return;
        }
        void ipc.hidePanel();
      }
    };

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
