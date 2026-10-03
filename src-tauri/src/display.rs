//! Where the panel is, and where it should go, in one coordinate space.
//!
//! macOS describes the same desk twice. **Quartz global display space** puts
//! the origin at the top-left of the main display with y growing downwards, in
//! *points*; `CGDisplayBounds` and `CGEvent::location` speak it. **AppKit
//! space** puts the origin at the bottom-left with y growing upwards;
//! `NSEvent::mouseLocation` and `NSScreen::frame` speak that.
//!
//! tao crosses the two, and that is what put the panel on the wrong screen:
//!
//! * `cursor_position()` flips an AppKit point with
//!   `CGDisplayPixelsHigh(main)`, a *point* height, not the pixel height the
//!   name suggests, and then multiplies the result by the primary display's
//!   backing scale factor. On the 2x built-in display this was found on, that
//!   doubles the cursor's coordinates.
//! * `monitor_from_point()` then tests those doubled coordinates against raw
//!   `CGDisplayBounds`, which are unscaled points. The point misses the display
//!   the cursor is actually on and matches the next one along.
//! * `available_monitors()` is no better: each display's Quartz origin scaled
//!   by that display's *own* factor, sitting next to a size that is already
//!   physical.
//!
//! So the cursor and the displays are read here, straight from Quartz, and the
//! arithmetic stays in points from end to end. Only the moving of the window is
//! left to tao, whose `LogicalPosition` happens to be expressed in this very
//! space, since `window_position` flips logical y with
//! `CGDisplayPixelsHigh(main)`, which is exactly the Quartz-to-AppKit flip.

use objc2_core_graphics::{CGDisplayBounds, CGError, CGEvent, CGGetActiveDisplayList};

/// Denominator of the fraction of a window that has to land on a display for
/// its position to be worth keeping. A window with only a sliver on-screen
/// cannot be grabbed again, so it counts as stale.
const MIN_VISIBLE_FRACTION: f64 = 3.0;

/// Where down a display the panel's top edge sits, as a fraction of the slack
/// it leaves.
const TOP_INSET_FRACTION: f64 = 0.28;

/// Past this many attached displays, something has gone wrong.
const MAX_DISPLAYS: usize = 16;

/// A rectangle in Quartz global display space: points, y increasing downwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Every attached display.
fn all() -> Vec<Rect> {
    // Two passes, the way CoreGraphics wants it: ask how many there are, then
    // ask for the ids. A display plugged in between the two calls just means
    // the second pass reports the larger number.
    let mut count = 0u32;
    unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &mut count) };

    let mut ids = [0u32; MAX_DISPLAYS];
    let wanted = count.min(MAX_DISPLAYS as u32);
    let mut found = 0u32;
    let err = unsafe { CGGetActiveDisplayList(wanted, ids.as_mut_ptr(), &mut found) };
    if err != CGError::Success {
        eprintln!("[display] could not list the displays: {err:?}");
        return Vec::new();
    }

    ids[..(found as usize).min(MAX_DISPLAYS)]
        .iter()
        .map(|id| {
            let bounds = CGDisplayBounds(*id);
            Rect::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            )
        })
        .collect()
}

/// Where the pointer is, in the same space as the displays.
pub fn cursor() -> Option<(f64, f64)> {
    // A null event is the sanctioned way to ask CoreGraphics for the pointer:
    // `CGEventCreate(NULL)` exists so that its location can be read.
    let event = CGEvent::new(None)?;
    let point = CGEvent::location(Some(&event));
    Some((point.x, point.y))
}

/// The display the cursor is on.
pub fn under_cursor() -> Option<Rect> {
    let (x, y) = cursor()?;
    containing(&all(), x, y)
}

/// Where a `w` x `h` window belongs if it is to sit centred on `screen` and a
/// little above the vertical middle of it.
pub fn placement_on(screen: Rect, w: f64, h: f64) -> (f64, f64) {
    let x = screen.x + (screen.w - w) / 2.0;
    let y = screen.y + (screen.h - h) * TOP_INSET_FRACTION;
    (x, y)
}

/// Whether enough of `rect` would land on an attached display to stay usable.
pub fn is_reachable(rect: Rect) -> bool {
    reachable_on(&all(), rect)
}

fn containing(displays: &[Rect], x: f64, y: f64) -> Option<Rect> {
    displays.iter().copied().find(|d| d.contains(x, y))
}

/// Whether more than a third of `rect`, on each axis, lands on one of
/// `displays`.
fn reachable_on(displays: &[Rect], rect: Rect) -> bool {
    displays.iter().any(|d| {
        let overlap_w = (rect.x + rect.w).min(d.x + d.w) - rect.x.max(d.x);
        let overlap_h = (rect.y + rect.h).min(d.y + d.h) - rect.y.max(d.y);

        overlap_w > rect.w / MIN_VISIBLE_FRACTION && overlap_h > rect.h / MIN_VISIBLE_FRACTION
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desk this bug was found on: a 2x built-in display, with a 1x
    /// external one beside it. Both tops are flush, which is why the external
    /// rectangle has no negative y here even though AppKit reports one for the
    /// same screen.
    fn builtin() -> Rect {
        Rect::new(0.0, 0.0, 1512.0, 982.0)
    }

    fn external() -> Rect {
        Rect::new(1512.0, 0.0, 2560.0, 1440.0)
    }

    fn desk() -> Vec<Rect> {
        vec![builtin(), external()]
    }

    /// The bug, in numbers. `cursor_position()` hands `monitor_from_point` a
    /// point already multiplied by the primary display's backing scale factor,
    /// which then gets tested against unscaled bounds, so the doubled point
    /// falls off the built-in display and lands on the external one.
    #[test]
    fn a_scaled_cursor_point_matches_the_wrong_display() {
        // NSEvent.mouseLocation, flipped the way tao flips it.
        let cursor = (962.9, 982.0 - 472.8);

        assert!(builtin().contains(cursor.0, cursor.1));
        assert!(!external().contains(cursor.0, cursor.1));

        let doubled = (cursor.0 * 2.0, cursor.1 * 2.0);
        assert!(!builtin().contains(doubled.0, doubled.1));
        assert!(
            external().contains(doubled.0, doubled.1),
            "the panel used to be centred here"
        );
    }

    /// ...and the same point, left alone, is unambiguously on the built-in one.
    #[test]
    fn a_cursor_on_the_builtin_display_picks_it() {
        let cursor = (962.9, 982.0 - 472.8);
        assert_eq!(containing(&desk(), cursor.0, cursor.1), Some(builtin()));
    }

    /// A cursor down at the bottom of the external display picks that one, so
    /// the fix is not simply "always prefer the first".
    #[test]
    fn a_cursor_on_the_external_display_picks_it() {
        assert_eq!(containing(&desk(), 2000.0, 1200.0), Some(external()));
    }

    #[test]
    fn centring_leaves_the_same_margin_on_both_sides() {
        let (x, y) = placement_on(builtin(), 760.0, 501.0);

        assert!((x - 376.0).abs() < 0.001, "x was {x}");
        assert!((y - 134.68).abs() < 0.001, "y was {y}");

        // Equal margins either side, and clear of both top and bottom.
        let left = x - builtin().x;
        let right = builtin().x + builtin().w - (x + 760.0);
        assert!((left - right).abs() < 0.001, "{left} against {right}");
        assert!(y > 0.0 && y + 501.0 < builtin().h);
    }

    /// A spot remembered on a display that has since been unplugged is stale.
    #[test]
    fn a_spot_on_a_missing_display_is_not_reachable() {
        let parked = Rect::new(1600.0, 300.0, 760.0, 501.0);

        assert!(reachable_on(&desk(), parked));
        assert!(!reachable_on(&[builtin()], parked));
    }

    /// Parked against the outer edge of the rightmost display: 100 points
    /// across is under a third of 760, so it cannot be grabbed back. The
    /// display has to be the rightmost one: a sliver of the left-hand display
    /// still overlaps the one beside it.
    #[test]
    fn a_sliver_of_the_panel_is_not_reachable() {
        let right_edge = external().x + external().w;

        let sliver = Rect::new(right_edge - 100.0, 300.0, 760.0, 501.0);
        assert!(!reachable_on(&desk(), sliver));

        let enough = Rect::new(right_edge - 300.0, 300.0, 760.0, 501.0);
        assert!(reachable_on(&desk(), enough));
    }
}
