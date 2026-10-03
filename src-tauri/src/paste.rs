//! The other half of "Paste to <app>".
//!
//! Writing the clip back to the pasteboard only makes the *content*
//! available; the target app still has to receive a Cmd+V. That takes two
//! steps we cannot get for free:
//!
//! 1. bringing the app that owned the frontmost window back to the front,
//!    and
//! 2. posting a synthetic keystroke, which macOS gates behind the
//!    Accessibility (TCC) permission.
//!
//! `is_trusted()` is therefore checked by the caller *before* the panel is
//! hidden, so a missing permission produces an actionable hint instead of a
//! panel that vanishes and an app that quietly does nothing.
//!
//! # Everything here is modelled on how "will it paste?" actually fails
//!
//! The naive version — activate the target, sleep a bit, `CGEventPost` to the
//! HID tap — fails silently against three very common targets, and each
//! failure looks identical from the outside: nothing happens.
//!
//! * **Activation is a request, not an order.** `activateWithOptions:` returns
//!   a bool and macOS is free to decline it, especially when the app asking is
//!   itself frontmost. So the only honest way to know is to *poll for it*: read
//!   who is frontmost, and ask again until they match or we run out of
//!   patience. A fixed sleep answers a question nobody asked.
//! * **The HID tap is not always the right tap.** Native apps that vend their
//!   own text system — Messages being the classic one — ignore events posted
//!   straight at the process. They want the *session* tap, which routes through
//!   the window server to whoever is currently frontmost. That is why the tap
//!   is chosen from the same reading the poll already took: if we got the app
//!   frontmost, use the session tap; if we did not, deliver directly to the pid
//!   and take what we can get.
//! * **The target may be gone.** A pid is a snapshot. `activateWithOptions:`
//!   on a dead process returns false, and we say so rather than pretending.

use std::thread;
use std::time::Duration;

/// Let our own window finish hiding and AppKit finish moving activation
/// before we start asking for it back. Asking during the handover loses a
/// race against AppKit's own idea of who should be active next.
const SETTLE_AFTER_HIDE: Duration = Duration::from_millis(180);

/// How long the poll runs for before it gives up and posts anyway. Ramped
/// rather than flat: a target that is slow to come forward usually needs the
/// time, not more repetitions.
const POLL_ATTEMPTS: u32 = 25;
const POLL_BASE: Duration = Duration::from_millis(50);
const POLL_STEP: Duration = Duration::from_millis(10);

/// `kVK_ANSI_V`. Virtual key codes address the *physical* key rather than the
/// character it produces, so Cmd+V stays correct on Dvorak / AZERTY.
const KEY_V: u16 = 9;

/// How long the whole poll can keep asking for: `POLL_ATTEMPTS` sleeps whose
/// length grows by `POLL_STEP` each round, i.e. Σ(base + step·i).
///
/// Derived rather than written down, because the obvious-looking spelling is
/// wrong by nearly a factor of three: `POLL_ATTEMPTS * (POLL_BASE + POLL_STEP)`
/// prices every round at its last value. Quote this constant instead of
/// re-deriving it anywhere else.
const fn wait_budget_ms() -> u32 {
    POLL_ATTEMPTS * POLL_BASE.as_millis() as u32
        + POLL_STEP.as_millis() as u32 * POLL_ATTEMPTS * (POLL_ATTEMPTS - 1) / 2
}

/// How [`wait_for_frontmost`] finished.
///
/// Worth distinguishing because the caller's fallback means two very
/// different things depending on which failure came back. A pid nobody is
/// running at any more cannot be pasted into by any route, and saying "no
/// running app" sends whoever is reading the log to the right place; a target
/// that merely took its time may yet be coming forward, and a direct post is
/// simply being asked to do the best it can.
enum Wait {
    /// `pid` really is frontmost now.
    Frontmost,
    /// Nothing is running at that pid any more; retrying is pointless.
    TargetGone,
    /// Never made it to the front before the budget ran out.
    TimedOut,
}

// `CGEventPostToPid`, which `objc2-core-graphics` does not bind. Delivering
// to a process directly is the fallback for targets that never made it to
// the front — there is nothing better available at that point, but there is
// also nothing worse about trying it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventPostToPid(pid: i32, event: *const std::ffi::c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    // Returns a CoreFoundation `Boolean` (unsigned char), not a Rust `bool`.
    fn AXIsProcessTrusted() -> u8;
}

/// Whether this process may post synthetic events and read the global key
/// stream. Everything else in the app works without it.
pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Drops the user straight on the Accessibility pane, the only place the
/// switch can be flipped. The deep link beats `AXIsProcessTrustedWithOptions`
/// because it also works on a second attempt, after the user has dismissed
/// the system prompt once.
pub fn open_accessibility_settings() {
    const PANE: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

    if let Err(err) = std::process::Command::new("open").arg(PANE).spawn() {
        eprintln!("[paste] could not open System Settings: {err}");
    }
}

/// Bring `pid` forward, then press Cmd+V.
///
/// Deliberately runs off the main thread: the settle delay and the whole
/// activation poll are part of the correctness, not politeness, and blocking
/// the UI thread for what can be a second while the window server tears down
/// our panel is exactly what makes the keystroke land in the wrong place.
pub fn deliver(pid: Option<i32>) {
    thread::Builder::new()
        .name("clipboard-paste".into())
        .spawn(move || paste_into(pid))
        .ok();
}

fn paste_into(pid: Option<i32>) {
    let Some(pid) = pid.filter(|p| *p > 0) else {
        // No remembered target — usually the first paste after launch, before
        // anything else has held focus. The content is already on the
        // pasteboard, and Cmd+V by hand still works.
        eprintln!("[paste] no target app remembered, skipping auto-paste");
        return;
    };

    thread::sleep(SETTLE_AFTER_HIDE);

    let restored = wait_for_frontmost(pid);

    // Chosen from the reading above, not guessed: see the module docs. Re-read
    // rather than trusting `restored`, because the app can lose the front
    // again in the instant between the poll ending and the post.
    #[cfg(target_os = "macos")]
    let via_session_tap = crate::frontmost::frontmost_pid() == Some(pid);
    #[cfg(not(target_os = "macos"))]
    let via_session_tap = false;

    // Every one of these still falls through to the post below; behaviour is
    // identical, only the explanation differs.
    match restored {
        Wait::Frontmost => {}
        Wait::TargetGone => eprintln!(
            "[paste] no running app at pid {pid}; delivering anyway, \
             though there is nothing there to receive it"
        ),
        Wait::TimedOut => eprintln!(
            "[paste] {pid} did not become frontmost within {}ms; delivering directly",
            wait_budget_ms()
        ),
    }

    if !press_command_v(pid, via_session_tap) {
        eprintln!("[paste] could not synthesise ⌘V for pid {pid}");
    }
}

/// Keeps asking until `pid` is genuinely frontmost, up to a budget of
/// [`wait_budget_ms`].
///
/// Re-activating inside the loop matters: macOS will sometimes decline the
/// first request while it is still animating our own panel away, and then
/// happily honour the second one.
#[cfg(target_os = "macos")]
fn wait_for_frontmost(pid: i32) -> Wait {
    use crate::frontmost::frontmost_pid;

    for attempt in 0..POLL_ATTEMPTS {
        if frontmost_pid() == Some(pid) {
            return Wait::Frontmost;
        }
        if !activate(pid) {
            // Reported by the caller, which knows what it is about to do with
            // the answer; logging here too would only double it.
            return Wait::TargetGone;
        }
        thread::sleep(POLL_BASE + POLL_STEP * attempt);
    }

    Wait::TimedOut
}

#[cfg(not(target_os = "macos"))]
fn wait_for_frontmost(_pid: i32) -> Wait {
    Wait::TimedOut
}

/// Hand the front to `pid`.
///
/// macOS 14 quietly retired the blunt instrument. `ActivateIgnoringOtherApps`
/// is documented as having no effect from that release on, so asking loudly
/// now buys nothing whatsoever while still reading like intent; asking for the
/// cooperative option alone is refused just as readily, because we are asking
/// from the app that currently holds the front.
///
/// What actually works is the coordinated form: the request names the
/// application the foreground is being taken *from*. That is the whole
/// difference — see the note above `activateFromApplication_options`.
#[cfg(target_os = "macos")]
fn activate(pid: i32) -> bool {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return false;
    };

    // macOS 14 changed the rules here, and quietly. `-activateWithOptions:`
    // still answers politely, but the `ActivateIgnoringOtherApps` variant is
    // documented as having no effect from that release on — so asking the
    // loud way buys nothing at all. What replaced it is coordinated
    // activation: the request has to name the app it is taking the foreground
    // *from*, which is what lets the system honour it.
    //
    // Hence the from-application variant, naming ourselves as the one yielding.
    // `ActivateAllWindows` rather than the ignoring flag, for the same reason.
    let this_app = NSRunningApplication::currentApplication();
    app.activateFromApplication_options(
        &this_app,
        NSApplicationActivationOptions::ActivateAllWindows,
    )
}

#[cfg(not(target_os = "macos"))]
fn activate(_pid: i32) -> bool {
    false
}

/// Post Cmd+V down+up.
///
/// `via_session_tap` is the branch the module docs argue for: once the target
/// really is frontmost, the session tap routes through the window server and
/// reaches apps — Messages chief among them — that ignore events aimed
/// straight at their process. Both taps at once would deliver the paste twice,
/// so it is one or the other, never both.
#[cfg(target_os = "macos")]
fn press_command_v(pid: i32, via_session_tap: bool) -> bool {
    use objc2_core_graphics::{
        CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    };

    // A real HID source (rather than a nil one) is what keeps busy apps,
    // Electron ones in particular, from ignoring the event.
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);

    let (Some(down), Some(up)) = (
        CGEvent::new_keyboard_event(source.as_deref(), KEY_V, true),
        CGEvent::new_keyboard_event(source.as_deref(), KEY_V, false),
    ) else {
        return false;
    };

    // The flags are what make this Cmd+V rather than a bare "v".
    CGEvent::set_flags(Some(&down), CGEventFlags::MaskCommand);
    CGEvent::set_flags(Some(&up), CGEventFlags::MaskCommand);

    if via_session_tap {
        CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&down));
        CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&up));
    } else {
        // SAFETY: both events are freshly created non-null `CGEventRef`s and
        // are not released by either call — `CGEventPostToPid` retains what it
        // needs. The pointer cast matches the C signature exactly.
        unsafe {
            CGEventPostToPid(pid, &*down as *const CGEvent as *const std::ffi::c_void);
            CGEventPostToPid(pid, &*up as *const CGEvent as *const std::ffi::c_void);
        }
    }

    true
}

#[cfg(not(target_os = "macos"))]
fn press_command_v(_pid: i32, _via_session_tap: bool) -> bool {
    false
}
