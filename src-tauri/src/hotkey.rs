//! Double-tap Option, the panel's second way in.
//!
//! macOS has no hotkey API for a *bare* modifier (`RegisterEventHotKey` wants
//! a real key), so this watches the global `flagsChanged` stream instead and
//! reconstructs "two quick lone Option taps" by hand.
//!
//! Two consequences worth knowing:
//!
//! * Global key monitoring is gated behind the same Accessibility (TCC)
//!   permission that `paste` needs. Without it the monitor never installs;
//!   we log that rather than pretending, and the regular hotkey still works.
//! * A held Option is indistinguishable from a slow tap *within* a single sample,
//!   so a tap is only credited when the release arrives quickly. Holding Option
//!   to type Option-combinations therefore never registers.
//!
//! And one that is easy to get wrong, because the name invites it:
//! `addGlobalMonitorForEventsMatchingMask` delivers copies of events destined
//! for *other applications* — Apple says so plainly. Fresh out of the gate we
//! *are* the frontmost application, with no window of ours to hand the
//! foreground to, so a tap on ⌥ is delivered straight to us and the global
//! monitor stays silent. That is the whole of "double-tap Option only starts
//! working after I click something else first". A second, *local* monitor
//! catches the events headed our way; between them every ⌥ is seen, whoever
//! owns the keyboard. It needs no permission either, being confined to our own
//! event stream — the local half works even before Accessibility is granted.

use std::ptr::NonNull;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

/// Longer than this and the user was *holding* Option, not tapping it.
const MAX_TAP: Duration = Duration::from_millis(250);
/// How long the second tap may wait after the first one ended.
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(320);
/// How often the Accessibility grant is re-checked while the app runs.
const TRUST_POLL: Duration = Duration::from_millis(1500);

/// The monitor tokens AppKit hands back, wrapped so they can live in a `static`.
///
/// `AnyObject` is deliberately neither `Send` nor `Sync`: objc2 will not vouch
/// for an opaque pointer it cannot see into. Everything that touches these
/// values happens on the main thread: they are created in `arm`, read in the
/// `run_on_main_thread` body of `watch_trust`, and handed straight back to
/// `removeMonitor`; and ObjC retain/release is atomic to begin with.
struct MonitorToken(Retained<AnyObject>);

// SAFETY: as documented above, the tokens never cross a thread boundary, and
// the mutex in `MONITOR` is only ever locked from the main thread.
unsafe impl Send for MonitorToken {}

/// The monitors currently installed, kept so a re-arm can retire the old ones.
///
/// AppKit owns the handler blocks; the tokens are what remain ours, and
/// dropping one without `removeMonitor` would leave a live tap behind. See
/// [`arm`] for why there are two rather than one.
static MONITOR: Mutex<Vec<MonitorToken>> = Mutex::new(Vec::new());

/// Install the monitor, then keep an eye on the permission behind it.
///
/// Idempotent: a second call is a no-op rather than a second monitor.
pub fn register(app: &AppHandle, state: &Arc<AppState>) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }

    arm(app, state);
    watch_trust(app.clone(), state.clone());
}

/// Install (or re-install) the double-tap monitors. Must run on the main
/// thread: this is AppKit, and so are the handlers it will call.
fn arm(app: &AppHandle, state: &Arc<AppState>) {
    let mut installed = Vec::with_capacity(2);
    installed.extend(install_global(app, state));
    installed.extend(install_local(app, state));

    if installed.is_empty() {
        eprintln!(
            "[hotkey] could not install the double-tap ⌥ monitors — \
             grant Accessibility to enable them"
        );
        return;
    }

    let retired = std::mem::replace(&mut *MONITOR.lock(), installed);

    for old in retired {
        // SAFETY: every token in this slot came from one install, and this is
        // the only place one is handed back.
        unsafe { NSEvent::removeMonitor(&old.0) };
    }

    // A non-nil token is *not* proof that the monitor will fire: without the
    // Accessibility grant macOS hands one back and then stays silent. Saying
    // so here beats debugging a deaf listener.
    if crate::paste::is_trusted() {
        println!("[hotkey] double-tap ⌥ armed (global + local)");
    } else {
        eprintln!(
            "[hotkey] double-tap ⌥ armed locally; the global monitor stays \
             inert until Accessibility is granted"
        );
    }
}

/// Watches the ⌥ taps headed for *other* applications.
///
/// Gated behind Accessibility, and silent without it — the token comes back
/// non-nil either way.
fn install_global(app: &AppHandle, state: &Arc<AppState>) -> Option<MonitorToken> {
    let (handle, shared) = (app.clone(), state.clone());

    let block = RcBlock::new(move |event: NonNull<NSEvent>| {
        let flags = unsafe { event.as_ref() }.modifierFlags().0;
        if observe(flags) {
            summon(&handle, &shared);
        }
    });

    let token =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged, &block)?;

    // The block must outlive the monitor, and the monitor outlives the
    // process, so there is never a later point at which to free it.
    std::mem::forget(block);
    Some(MonitorToken(token))
}

/// Watches the ⌥ taps headed for us.
///
/// The other half of the story: while we hold the foreground — every moment
/// this app has no window open, which is most of them — `flagsChanged` events
/// are ours and never pass through the global monitor at all. Confined to our
/// own event stream, so unlike its sibling it needs no permission.
fn install_local(app: &AppHandle, state: &Arc<AppState>) -> Option<MonitorToken> {
    let (handle, shared) = (app.clone(), state.clone());

    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let flags = unsafe { event.as_ref() }.modifierFlags().0;
        if observe(flags) {
            summon(&handle, &shared);
        }

        // Handed straight back, unwatched rather than intercepted: returning
        // null here would swallow the very keypress we are listening for.
        event.as_ptr()
    });

    let token = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged, &block)
    }?;

    std::mem::forget(block);
    Some(MonitorToken(token))
}

/// Opens (or closes) the panel for a recognised double tap, off AppKit's clock.
fn summon(handle: &AppHandle, shared: &Arc<AppState>) {
    // Never open a window from inside AppKit's event dispatch; defer to the
    // next pass of the main loop instead.
    let (dispatched_handle, dispatched_state) = (handle.clone(), shared.clone());
    let queued = handle
        .run_on_main_thread(move || {
            crate::window::toggle_panel(&dispatched_handle, &dispatched_state);
        })
        .is_ok();

    if !queued {
        // Only reachable if the event loop is already gone, but a silent
        // no-op would be worse than a direct call.
        crate::window::toggle_panel(handle, shared);
    }
}

/// Watch the Accessibility grant for as long as the process lives.
///
/// This exists because a monitor installed *before* the grant is handed back
/// non-nil and then stays silent forever; flipping the switch in System
/// Settings does not wake it up. Granting the permission is something only
/// the user can do, so the least we can do is notice when they have done it
/// instead of making them restart the app on top of it.
fn watch_trust(app: AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("clipboard-trust".into())
        .spawn(move || {
            let mut granted = crate::paste::is_trusted();

            loop {
                std::thread::sleep(TRUST_POLL);

                let now = crate::paste::is_trusted();
                if now == granted {
                    continue;
                }
                granted = now;

                let (handle, shared) = (app.clone(), state.clone());
                let _ = app.run_on_main_thread(move || {
                    if now {
                        arm(&handle, &shared);
                    } else {
                        // Revoked mid-flight: retire the monitors rather than
                        // leave listeners that can no longer hear anything.
                        let retired = std::mem::take(&mut *MONITOR.lock());

                        for old in retired {
                            // SAFETY: every token came from one install, and
                            // this is only ever handed back here.
                            unsafe { NSEvent::removeMonitor(&old.0) };
                        }

                        eprintln!("[hotkey] Accessibility revoked — double-tap ⌥ disabled");
                    }
                });

                // The panel's "grant Accessibility" hint is stale the moment
                // the switch flips, whichever way it went.
                let _ = app.emit("accessibility://changed", now);
            }
        })
        .ok();
}

struct Tracker {
    /// Set while a lone Option is held.
    down_since: Option<Instant>,
    /// When the previous credited tap ended.
    last_tap_end: Option<Instant>,
}

impl Tracker {
    const fn new() -> Self {
        Self {
            down_since: None,
            last_tap_end: None,
        }
    }
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker::new());

/// Feeds one `flagsChanged` sample in and reports whether it completed a
/// double tap. A dropped tap costs the user a panel that did not open, while
/// whatever panicked inside the previous sample has already been dealt with
/// elsewhere — so these locks use `parking_lot`, which has no poisoning to
/// recover from in the first place.
fn observe(raw: usize) -> bool {
    let mut tracker = TRACKER.lock();

    if lone_option(raw) {
        // `get_or_insert_with` so an extra press sample can't restart the
        // clock and turn a hold into a tap.
        tracker.down_since.get_or_insert_with(Instant::now);
        return false;
    }

    let Some(pressed_at) = tracker.down_since.take() else {
        // Some *other* modifier changed. Anything pending is off: the user is
        // building a chord, not tapping.
        tracker.last_tap_end = None;
        return false;
    };

    let now = Instant::now();
    if now.duration_since(pressed_at) > MAX_TAP {
        tracker.last_tap_end = None;
        return false;
    }

    match tracker.last_tap_end.take() {
        Some(previous) if now.duration_since(previous) <= DOUBLE_TAP_WINDOW => true,
        // First tap of a potential pair.
        _ => {
            tracker.last_tap_end = Some(now);
            false
        }
    }
}

/// "Exactly Option and nothing else is down."
fn lone_option(raw: usize) -> bool {
    // The low half of the raw flags carries the left/right variants of each
    // modifier, so collapse to the device-independent bits first.
    let independent = raw & NSEventModifierFlags::DeviceIndependentFlagsMask.0;

    // These never mean "the user is pressing Option with something else", and
    // leaving them in would silently break double-tap Option whenever caps lock is
    // on, which is a maddening bug to chase down later.
    let irrelevant = NSEventModifierFlags::CapsLock.0
        | NSEventModifierFlags::Function.0
        | NSEventModifierFlags::NumericPad.0
        | NSEventModifierFlags::Help.0;

    independent & !irrelevant == NSEventModifierFlags::Option.0
}
