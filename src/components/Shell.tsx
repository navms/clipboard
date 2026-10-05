import { cn } from "../lib/cn";
import { isTauri } from "../lib/ipc";
import { ActionsMenu } from "./ActionsMenu";
import { BottomBar } from "./BottomBar";
import { DetailPanel } from "./DetailPanel";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

const PANEL_W = 760;
const PANEL_H = 480;

/** The app chrome itself: top bar / list + detail / bottom bar. */
function Panel() {
  return (
    <div
      className={cn(
        // `rounded-panel` (10px) is the window corner; `shadow-panel` is the
        // top of the elevation scale and belongs to the window alone.
        "relative flex h-full w-full flex-col overflow-hidden rounded-panel bg-app text-ink ring-1 ring-panel-ring",
        // Inside Tauri the window is transparent and macOS draws the drop
        // shadow; a CSS shadow would simply be clipped at the window edge.
        !isTauri && "shadow-panel",
      )}
    >
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <DetailPanel />
      </div>
      <BottomBar />
      <ActionsMenu />
    </div>
  );
}

/**
 * Inside Tauri the panel fills the transparent, always-on-top window.
 * In a plain browser it is centred in a fixed-size frame matching the real
 * window dimensions, so the layout can be checked at its actual size.
 */
export function Shell() {
  if (isTauri) return <Panel />;

  return (
    <div className="flex h-full w-full items-center justify-center p-6">
      <div
        className="max-h-full max-w-full overflow-hidden rounded-panel"
        style={{ width: PANEL_W, height: PANEL_H }}
      >
        <Panel />
      </div>
    </div>
  );
}
