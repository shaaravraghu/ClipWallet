//! macOS CGEventTap — a two-tier key listener (issue #27).
//!
//! Tier 1 (Idle) is the lightweight listener that always runs: it watches
//! `FlagsChanged` for the qualifying chord modifiers (Command, Option) and does
//! the absolute minimum on `KeyDown`/`KeyUp`. While Idle — i.e. during ordinary
//! typing — key events are passed straight through without touching the chord
//! detector at all.
//!
//! Tier 2 (Active) is the full chord evaluator. It is engaged the moment a
//! qualifying modifier goes down and disengaged only once BOTH qualifying
//! modifiers are released (see [`chord::tier_active`]). The `active` gate is the
//! boundary between the two tiers.
//!
//! Design notes, keyed to review feedback on the implementation plan:
//!   1. Idle transition is derived from the *complete* masked modifier state,
//!      so a KeyDown landing between two modifier-release events is never
//!      processed against a half-released chord (`FlagsChanged` arm).
//!   2. `ModState` is masked strictly to Command|Option, so a Caps Lock / Shift
//!      / Control toggle cannot perturb the gate (`mask_qualifying`).
//!   3. On `kCGEventTapDisabledByTimeout` the tap is re-enabled AND modifier
//!      state is re-synced from the live event source before the handler
//!      returns, closing the stale-state window (`resync_after_reenable`).
//!   4. The gate is an `AtomicBool` read `Relaxed` on the hot path — justified
//!      in the comment on `EventTapState::active`.
//!   6. Events we inject carry a tag in `EVENT_SOURCE_USER_DATA`; they are
//!      short-circuited at the very top of `handle_event` so they cannot flip
//!      the tier (`SYNTHETIC_TAG`).
//!
//! The detector also keeps the original modifier-first-release handling: a
//! chord letter whose KeyUp trails the modifier release still fires, even if
//! that KeyUp arrives after the tier has fallen back to Idle.

use crate::hotkey::chord::tier_active;
use crate::hotkey::{ChordDetector, HotkeyAction, SYNTHETIC_TAG};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopRef, CFRunLoopStop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, EventField,
};
use core_graphics::event_source::CGEventSourceStateID;
use std::cell::Cell;
use std::cell::Cell;
use std::cell::RefCell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use tracing::{debug, info, warn};

// CoreGraphics symbols the `core-graphics` crate does not wrap. CoreGraphics is
// already linked transitively through that crate, so we only need declarations.
extern "C" {
    /// Current modifier flags for the given event-source state. Used to re-sync
    /// after the tap is re-enabled (issue #27, review pt 3).
    fn CGEventSourceFlagsState(state_id: CGEventSourceStateID) -> u64;
    /// Enable / disable an event tap by its mach port. Needed to re-arm the tap
    /// from inside its own callback, where the `CGEventTap` value is not
    /// borrowable.
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// Mutable state shared by every invocation of the tap callback. The CGEventTap
/// callback is an `Fn`, so all fields use interior mutability.
struct EventTapState {
    detector: RefCell<ChordDetector>,

    /// Cached modifier state, already masked to ONLY Command|Option
    /// (issue #27, review pt 2). Every other CGEventFlags bit — Caps Lock
    /// (AlphaShift), Shift, Control, Fn, NumericPad, NonCoalesced … — is
    /// excluded, so toggling one of them produces no change here and therefore
    /// no spurious tier transition.
    mod_state: Cell<CGEventFlags>,

    /// The tier gate. `true` == Active (tier 2), `false` == Idle (tier 1).
    ///
    /// Memory ordering (issue #27, review pt 4): every access is `Relaxed`.
    /// This is correct and intentional — do NOT "upgrade" it to `Acquire`/
    /// `Release`/`SeqCst` on the assumption that stricter is safer. The event
    /// tap callback runs on a single thread (the grabber run loop), which is
    /// the only writer and the only reader on the hot path; there is no second
    /// thread and no companion data being published or acquired *through* this
    /// flag, so ordering relative to other memory operations is irrelevant.
    /// `Relaxed` gives the cheapest correct load on the per-keystroke fast
    /// path. (It is an atomic rather than a `Cell<bool>` only so the gate stays
    /// trivially safe to observe should a future read ever occur off-thread.)
    active: AtomicBool,
}

/// Owned by main; calling `.stop()` tears down the CGEventTap thread.
#[derive(Clone)]
pub struct GrabberHandle {
    run_loop: Arc<RunLoopHandle>,
    suppressed: Arc<AtomicBool>,
}

struct RunLoopHandle(CFRunLoopRef);
unsafe impl Send for RunLoopHandle {}
unsafe impl Sync for RunLoopHandle {}

impl GrabberHandle {
    pub fn stop(&self) {
        self.suppressed.store(true, Ordering::SeqCst);
        unsafe {
            CFRunLoopStop(self.run_loop.0);
        }
    }
}

/// Spawn the event tap on its own OS thread and return a stoppable handle.
/// Replaces the previous fire-and-forget `start_event_tap` call site.
pub fn spawn_event_tap(tx: SyncSender<HotkeyAction>, mode_static: bool) -> GrabberHandle {
    let (loop_tx, loop_rx) = sync_channel::<RunLoopHandle>(1);
    let suppressed = Arc::new(AtomicBool::new(false));
    let suppressed_thread = suppressed.clone();

    std::thread::Builder::new()
        .name("clipwallet-grabber".into())
        .spawn(move || {
            start_event_tap_inner(tx, mode_static, loop_tx, suppressed_thread);
        })
        .expect("spawn grabber thread");

    let run_loop_handle = loop_rx
        .recv()
        .expect("grabber thread failed before publishing its run loop");

    GrabberHandle {
        run_loop: Arc::new(run_loop_handle),
        suppressed,
    }
}

fn start_event_tap_inner(
    tx: SyncSender<HotkeyAction>,
    mode_static: bool,
    loop_tx: std::sync::mpsc::SyncSender<CFRunLoopRef>,
    suppressed: Arc<AtomicBool>,
) {
    let state = Rc::new(EventTapState {
        detector: RefCell::new(ChordDetector::new()),
        mod_state: Cell::new(CGEventFlags::empty()),
        active: AtomicBool::new(false),
    });

    // The callback needs to re-enable the tap on timeout, but the `CGEventTap`
    // is not borrowable from inside its own closure. Share a slot for the tap's
    // mach port; we fill it immediately after creation, long before the run
    // loop delivers the first event.
    let tap_port: Rc<Cell<Option<CFMachPortRef>>> = Rc::new(Cell::new(None));

    let state_cb = Rc::clone(&state);
    let tap_port_cb = Rc::clone(&tap_port);
    let suppressed_cb = suppressed.clone();

    let tap_port: Rc<Cell<Option<CFMachPortRef>>> = Rc::new(Cell::new(None));
    let tap_port_cb = Rc::clone(&tap_port);

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        move |_proxy, event_type, event| {
            if suppressed_cb.load(Ordering::Relaxed) {
                return Some(event.to_owned());
            }

            match event_type {
                // ── Tap disabled: re-enable and re-sync BEFORE returning ──
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    if let Some(port) = tap_port_cb.get() {
                        unsafe {
                            CGEventTapEnable(port, true);
                        }
                    }
                    resync_after_reenable(&state_cb);
                    None
                }
                _ => handle_event(&state_cb, &tx, event_type, event, mode_static),
            }
        },
    )
    .expect("CGEventTap creation failed — ensure Accessibility permission is granted");

    // Make the port reachable from the callback for the timeout path above.
    tap_port.set(Some(tap.mach_port.as_concrete_TypeRef()));

    let loop_src = tap
        .mach_port
        .create_runloop_source(0)
        .expect("Failed to create CFRunLoop source");

    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&loop_src, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    let _ = loop_tx.send(RunLoopHandle(run_loop.as_concrete_TypeRef()));
    info!("CGEventTap active — two-tier key listener running (starts Idle)");
    CFRunLoop::run_current();
    info!("CGEventTap stopped");
}

/// Strictly mask raw `CGEventFlags` down to the two qualifying chord modifiers.
/// Anything else (Caps Lock, Shift, Control, Fn, …) is dropped so it can never
/// reach `ModState` (issue #27, review pt 2).
#[inline]
fn mask_qualifying(flags: CGEventFlags) -> CGEventFlags {
    let mut masked = CGEventFlags::empty();
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        masked.insert(CGEventFlags::CGEventFlagCommand);
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        masked.insert(CGEventFlags::CGEventFlagAlternate);
    }
    masked
}

/// Whether a masked modifier state should keep tier 2 engaged. Delegates to the
/// single source of truth for the gate rule (issue #27, review pt 1).
#[inline]
fn qualifies(masked: CGEventFlags) -> bool {
    tier_active(
        masked.contains(CGEventFlags::CGEventFlagCommand),
        masked.contains(CGEventFlags::CGEventFlagAlternate),
    )
}

/// Re-arm tier state after the tap was disabled (timeout / user input).
///
/// Reads the live modifier flags straight from the event source and rebuilds
/// `ModState`, the detector's modifier view, and the gate — all before the
/// callback returns. Without this, a KeyDown arriving immediately after
/// re-enable (and before the first fresh `FlagsChanged`) would be gated on
/// stale state (issue #27, review pt 3).
fn resync_after_reenable(state: &EventTapState) {
    let mut detector = state.detector.borrow_mut();

    // Drop any held-key state accumulated before the tap went dark; KeyUps may
    // have been missed while it was disabled (issue #27, review pt 5).
    detector.reset();

    let raw = unsafe { CGEventSourceFlagsState(CGEventSourceStateID::CombinedSessionState) };
    let live = CGEventFlags::from_bits_truncate(raw);
    let masked = mask_qualifying(live);

    state.mod_state.set(masked);
    sync_modifiers(&mut detector, live);
    state.active.store(qualifies(masked), Ordering::Relaxed);

    warn!(
        "Event tap re-enabled after disable — modifier state re-synced (cmd={}, opt={})",
        masked.contains(CGEventFlags::CGEventFlagCommand),
        masked.contains(CGEventFlags::CGEventFlagAlternate),
    );
}

fn handle_event(
    state: &EventTapState,
    tx: &SyncSender<HotkeyAction>,
    event_type: CGEventType,
    event: &CGEvent,
    mode_static: bool,
) -> Option<CGEvent> {
    // ── Synthetic-event short-circuit (issue #27, review pt 6) ────────────
    // Anything we injected (copy / cut / paste) carries SYNTHETIC_TAG. Pass it
    // through untouched: it must never drive a tier transition or be read as a
    // chord. Checked first, for every event type, so no injection path leaks.
    if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == SYNTHETIC_TAG {
        return Some(event.to_owned());
    }

    match event_type {
        // ── FlagsChanged: the tier-1 listener ─────────────────────────────
        CGEventType::FlagsChanged => {
            let flags = event.get_flags();
            let masked = mask_qualifying(flags);
            let prev = state.mod_state.replace(masked);

            // Gate from the COMPLETE masked state, not from the single bit this
            // event toggled (issue #27, review pt 1). One qualifying bit still
            // set ⇒ stay Active; only an empty masked state falls to Idle.
            let now_active = qualifies(masked);
            let was_active = state.active.swap(now_active, Ordering::Relaxed);

            // Keep the full evaluator's modifier view current. FlagsChanged is
            // not the hot path (it fires only on modifier transitions), so this
            // is cheap. Pending chord letters are retained across release here,
            // preserving the modifier-first-release behavior.
            sync_modifiers(&mut state.detector.borrow_mut(), flags);

            if was_active != now_active {
                debug!("Tier → {}", if now_active { "Active" } else { "Idle" });
            }
            if masked != prev {
                debug!("ModState {:?} → {:?}", prev, masked);
            }
            Some(event.to_owned())
        }

        // ── KeyDown ───────────────────────────────────────────────────────
        CGEventType::KeyDown => {
            // Tier-1 fast path: while Idle, do nothing and forward the event.
            // Relaxed load is correct here — see EventTapState::active.
            if !state.active.load(Ordering::Relaxed) {
                return Some(event.to_owned());
            }

            // ── Tier-2 (Active): full chord evaluation ──
            let mut detector = state.detector.borrow_mut();

            // Filter autorepeat inside an active chord.
            let is_repeat =
                event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) == 1;
            if is_repeat && detector.cmd() && detector.opt() {
                return None;
            }

            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let key = match keycode_to_rdev(keycode) {
                Some(k) => k,
                None => return Some(event.to_owned()),
            };

            // Update detector held set FIRST so digit actions see C/X/V.
            detector.key_down(key.clone());
            debug!(
                "KeyDown {:?}  cmd={} opt={}",
                key,
                detector.cmd(),
                detector.opt()
            );

            if detector.cmd() && detector.opt() {
                if let Some(action) = detector.evaluate_press(&key, mode_static) {
                    debug!("Action (press): {:?}", action);
                    let _ = tx.send(action);
                }
                return None; // suppress user chord keydown
            }

            Some(event.to_owned())
        }

        // ── KeyUp ───────────────────────────────────────────────────────
        CGEventType::KeyUp => {
            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let key = match keycode_to_rdev(keycode) {
                Some(k) => k,
                None => return Some(event.to_owned()),
            };

            let mut detector = state.detector.borrow_mut();

            if !state.active.load(Ordering::Relaxed) {
                // Tier-1 (Idle). Almost always a no-op — but a chord letter
                // whose trailing KeyUp lands AFTER both modifiers were released
                // is still pending and must fire, so the modifier-first-release
                // behavior survives an Active→Idle transition.
                if detector.is_pending_chord_key(&key) {
                    if let Some(action) = detector.evaluate_release(&key, mode_static) {
                        debug!("Action (release, idle-trailing): {:?}", action);
                        let _ = tx.send(action);
                        detector.key_up(key);
                        return None;
                    }
                }
                detector.key_up(key);
                return Some(event.to_owned());
            }

            // ── Tier-2 (Active) ──
            // Use the detector's OWN modifier state, not live flags: live flags
            // can already show Opt=false by the time the letter's KeyUp fires.
            let was_active = detector.cmd() && detector.opt();
            let is_pending = detector.is_pending_chord_key(&key);
            debug!(
                "KeyUp {:?}  detector_active={}  pending={}",
                key, was_active, is_pending
            );

            if was_active || is_pending {
                if let Some(action) = detector.evaluate_release(&key, mode_static) {
                    debug!("Action (release): {:?}", action);
                    let _ = tx.send(action);
                    detector.key_up(key);
                    return None; // suppress user chord keyup
                }
            }

            detector.key_up(key);

            // Suppress the raw event only if the chord was active.
            if was_active {
                return None;
            }
            Some(event.to_owned())
        }

        _ => Some(event.to_owned()),
    }
}

fn sync_modifiers(detector: &mut ChordDetector, flags: CGEventFlags) {
    use rdev::Key;
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        detector.key_down(Key::MetaLeft);
    } else {
        detector.key_up(Key::MetaLeft);
        detector.key_up(Key::MetaRight);
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        detector.key_down(Key::Alt);
    } else {
        detector.key_up(Key::Alt);
        detector.key_up(Key::AltGr);
    }
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        detector.key_down(Key::ShiftLeft);
    } else {
        detector.key_up(Key::ShiftLeft);
        detector.key_up(Key::ShiftRight);
    }
}

fn keycode_to_rdev(code: u16) -> Option<rdev::Key> {
    use rdev::Key::*;
    Some(match code {
        0 => KeyA,
        1 => KeyS,
        2 => KeyD,
        3 => KeyF,
        4 => KeyH,
        5 => KeyG,
        6 => KeyZ,
        7 => KeyX,
        8 => KeyC,
        9 => KeyV,
        11 => KeyB,
        12 => KeyQ,
        13 => KeyW,
        14 => KeyE,
        15 => KeyR,
        16 => KeyY,
        17 => KeyT,
        31 => KeyO,
        32 => KeyU,
        34 => KeyI,
        35 => KeyP,
        37 => KeyL,
        38 => KeyJ,
        40 => KeyK,
        45 => KeyN,
        46 => KeyM,

        18 => Num1,
        19 => Num2,
        20 => Num3,
        21 => Num4,
        23 => Num5,
        22 => Num6,
        26 => Num7,
        28 => Num8,
        25 => Num9,
        29 => Num0,

        48 => Tab,
        53 => Escape,
        36 => Return,
        51 => Backspace,
        49 => Space,
        123 => LeftArrow,
        124 => RightArrow,
        125 => DownArrow,
        126 => UpArrow,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_strips_non_qualifying_bits() {
        let polluted = CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagAlphaShift   // Caps Lock
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl;
        let m = mask_qualifying(polluted);
        assert!(m.contains(CGEventFlags::CGEventFlagCommand));
        assert!(m.contains(CGEventFlags::CGEventFlagAlternate));
        assert!(!m.contains(CGEventFlags::CGEventFlagAlphaShift));
        assert!(!m.contains(CGEventFlags::CGEventFlagShift));
        assert!(!m.contains(CGEventFlags::CGEventFlagControl));
    }

    #[test]
    fn caps_lock_toggle_does_not_change_masked_state() {
        // Cmd held, then Caps Lock toggles: the masked state is identical, so
        // the tier gate sees no change and cannot fire spuriously (review pt 2).
        let before = mask_qualifying(CGEventFlags::CGEventFlagCommand);
        let after =
            mask_qualifying(CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagAlphaShift);
        assert_eq!(before, after);
        assert!(qualifies(after));
    }

    #[test]
    fn gate_engages_on_either_modifier_only() {
        assert!(qualifies(mask_qualifying(CGEventFlags::CGEventFlagCommand)));
        assert!(qualifies(mask_qualifying(
            CGEventFlags::CGEventFlagAlternate
        )));
        // Non-qualifying modifiers alone must NOT engage tier 2.
        assert!(!qualifies(mask_qualifying(CGEventFlags::CGEventFlagShift)));
        assert!(!qualifies(mask_qualifying(
            CGEventFlags::CGEventFlagControl
        )));
        assert!(!qualifies(mask_qualifying(CGEventFlags::empty())));
    }
}
