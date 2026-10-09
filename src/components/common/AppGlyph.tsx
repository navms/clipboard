import { cn } from "../../lib/cn";

// The bundle icon itself (public/app-icon.png); the drop-shadow traces the
// artwork's alpha so its white tile still reads on the panel's white footer.
export function AppGlyph({ className }: { className?: string }) {
  return (
    <img
      src="/app-icon.png"
      alt=""
      aria-hidden
      draggable={false}
      className={cn(
        "h-4 w-4 shrink-0 select-none drop-shadow-[0_0_0.5px_rgba(0,0,0,0.5)]",
        className,
      )}
    />
  );
}
