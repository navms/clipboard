import { useEffect } from "react";
import { useApp } from "../stores/useApp";
import { isTauri } from "../lib/ipc";

/**
 * Bridges Rust-side push events into the store.
 *
 *   clip://changed           -> a capture landed, re-query the list
 *   panel://shown            -> panel summoned; reset filters, focus search
 *   accessibility://changed  -> the TCC grant was toggled while we ran
 *
 * A no-op in the browser preview.
 */
export function useClipEvents() {
  useEffect(() => {
    if (!isTauri) return;

    let disposed = false;
    const unlisteners: Array<() => void> = [];

    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");

      const onChanged = await listen("clip://changed", () => {
        void useApp.getState().refresh();
      });

      const onShown = await listen<{ targetApp?: string | null }>(
        "panel://shown",
        (event) => {
          useApp.setState({
            query: "",
            filter: "all",
            // Land on the timeline, same as the type filter: a freshly
            // summoned panel should look the same every time.
            view: "history",
            actionsOpen: false,
            typeFilterOpen: false,
            targetApp: event.payload?.targetApp ?? null,
          });
          void useApp
            .getState()
            .refresh()
            .then(() => window.dispatchEvent(new Event("rc:focus-search")));
        },
      );

      // Only a *grant* is actionable here: the hint is about the permission
      // being missing, so it retires itself the moment one arrives. The
      // opposite direction has nothing to add; the next paste says it better.
      const onAccessibility = await listen<boolean>(
        "accessibility://changed",
        (event) => {
          if (event.payload) useApp.setState({ pasteBlocked: false });
        },
      );

      if (disposed) {
        onChanged();
        onShown();
        onAccessibility();
        return;
      }
      unlisteners.push(onChanged, onShown, onAccessibility);
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((off) => off());
    };
  }, []);
}
