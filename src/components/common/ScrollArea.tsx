import { useCallback, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { cn } from "../../lib/cn";

/** Idle time before the thumb starts fading out, in ms. */
const FADE_MS = 700;
/** How long the fade itself runs, in ms. Deliberately unhurried: a fast
 *  snap-away draws the eye, a slow dissolve reads as calm. */
const FADE_DURATION_MS = 500;
/** The thumb never shrinks below this, so long lists keep a grabbable target. */
const MIN_THUMB_H = 28;

interface Props {
  /**
   * Optional handle on the scrolling element: the virtualiser needs one.
   * Omit it when nothing outside the component has to touch the scroller.
   */
  scrollRef?: RefObject<HTMLDivElement | null>;
  /** Applied to the scrolling element itself (padding, `relative`, ...). */
  className?: string;
  children: ReactNode;
}

/**
 * A scroll container with a **self-drawn** overlay scrollbar.
 *
 * Why not let the platform draw it? On macOS the native scroller is only an
 * *overlay* when the system happens to be configured that way. With a mouse
 * attached, or `AppleShowScrollBars=Always`, WebKit falls back to the
 * *classic* variant: a permanent 8px grey thumb inside a visible track, which
 * is both wider and louder than we want. And the instant you try to slim
 * it down with `scrollbar-width` / `scrollbar-color` / `::-webkit-scrollbar`,
 * you give up the fade-out for good, because those properties pin the scroller
 * into classic mode.
 *
 * So we stop asking: hide the native scroller outright and draw a macOS-style
 * thumb ourselves. That also buys back the behaviour we want: present while
 * scrolling, gone once you stop.
 */
export function ScrollArea({ scrollRef: externalRef, className, children }: Props) {
  const innerRef = useRef<HTMLDivElement>(null);
  const scrollRef = externalRef ?? innerRef;

  const [thumb, setThumb] = useState<{ top: number; height: number } | null>(null);
  const [active, setActive] = useState(false);
  const fadeTimer = useRef<number | undefined>(undefined);
  const frame = useRef<number | undefined>(undefined);

  const sync = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;

    const { scrollTop, scrollHeight, clientHeight } = el;
    if (scrollHeight <= clientHeight + 1) {
      setThumb((prev) => (prev === null ? prev : null));
      return;
    }

    const height = Math.min(
      clientHeight,
      Math.max(MIN_THUMB_H, (clientHeight / scrollHeight) * clientHeight),
    );
    const maxTop = clientHeight - height;
    const travel = scrollHeight - clientHeight;

    // Rubber-band overscroll drives `scrollTop` outside its own range:
    // negative when you yank past the top, beyond `travel` at the bottom.
    // Scaling that raw value would fling the thumb clean out of the pane (it
    // ends up drawn over the top bar), so clamp it to the track. This is also
    // what the native scroller does: during a bounce the thumb simply parks
    // at the end rather than travelling with the rubber.
    const rawTop = travel > 0 ? maxTop * (scrollTop / travel) : 0;
    const top = Math.min(maxTop, Math.max(0, rawTop));

    // Bail out when nothing actually moved. The effect below re-measures on
    // every render, so handing React a fresh object here unconditionally
    // would schedule another render, and another: an endless loop that shows
    // up as stutter while scrolling.
    setThumb((prev) =>
      prev !== null &&
      Math.abs(prev.top - top) < 0.5 &&
      Math.abs(prev.height - height) < 0.5
        ? prev
        : { top, height },
    );
  }, [scrollRef]);

  const handleScroll = useCallback(() => {
    setActive(true);
    window.clearTimeout(fadeTimer.current);
    fadeTimer.current = window.setTimeout(() => setActive(false), FADE_MS);

    // Scroll fires far faster than we need to reposition the thumb; coalesce
    // to one measurement per frame.
    if (frame.current !== undefined) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = undefined;
      sync();
    });
  }, [sync]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    el.addEventListener("scroll", handleScroll, { passive: true });
    // The viewport can change without a scroll (window resize, sidebar
    // collapse); content height changes are caught by the render-effect below.
    const observer = new ResizeObserver(sync);
    observer.observe(el);

    return () => {
      el.removeEventListener("scroll", handleScroll);
      observer.disconnect();
      window.clearTimeout(fadeTimer.current);
      if (frame.current !== undefined) cancelAnimationFrame(frame.current);
    };
  }, [scrollRef, handleScroll, sync]);

  // Re-measure after every render: rows arrive async and the type filter can
  // shrink the list to nothing, neither of which moves the viewport.
  useEffect(sync);

  return (
    // `overflow-hidden` is a deliberate second line of defence: the thumb is
    // absolutely positioned, so *any* bad number in `sync` would paint it
    // outside the pane, which is exactly how the bounce bug showed up (the
    // bar overhanging the top edge). The clamp above is the real fix; this
    // just makes the failure mode structurally impossible.
    <div className="relative min-h-0 flex-1 overflow-hidden">
      <div
        ref={scrollRef}
        className={cn(
          // `overscroll-contain` stops the scroll from chaining out to the
          // window once this list hits its end; without it the whole panel
          // rubber-bands as you keep scrolling.
          "h-full overflow-y-auto overscroll-contain",
          // Belt and braces: `scrollbar-width` covers Chrome/Firefox, the
          // pseudo-element covers older WebKit. Both are needed, and both only
          // ever *hide*, never style, so we stay out of classic mode.
          "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          className,
        )}
      >
        {children}
      </div>

      {thumb !== null && (
        <div
          aria-hidden
          data-scroll-thumb
          className="pointer-events-none absolute right-0.5 w-1.5 rounded-full bg-ink/25 transition-opacity ease-out"
          style={{
            top: thumb.top,
            height: thumb.height,
            opacity: active ? 1 : 0,
            // Snap on, fade off: the usual scroll-indicator asymmetry.
            transitionDuration: active ? "0ms" : `${FADE_DURATION_MS}ms`,
          }}
        />
      )}
    </div>
  );
}
