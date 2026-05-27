//! macOS CGEventTap — intercepts and suppresses key events.
//! Uses two complementary strategies to avoid the modifier-first-release race:
//!   1. Uses detector's OWN modifier state (not live CGEventFlags) on KeyUp.
//!   2. Also fires evaluate_release for any key still pending in
//!      keys_pressed_in_chord even when was_active is false — covers the case
//!      where Opt's FlagsChanged fires before C's KeyUp arrives.

use crate::hotkey::{ChordDetector, HotkeyAction};
use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField,
};
use std::cell::RefCell;
use std::sync::mpsc::SyncSender;
use tracing::{debug, info};

pub fn start_event_tap(tx: SyncSender<HotkeyAction>, mode_static: bool) {
    let detector      = RefCell::new(ChordDetector::new());
    let ignore_next_c = RefCell::new(false);
    let ignore_next_x = RefCell::new(false);

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
            handle_event(
                &mut detector.borrow_mut(),
                &mut ignore_next_c.borrow_mut(),
                &mut ignore_next_x.borrow_mut(),
                &tx,
                event_type,
                event,
                mode_static,
            )
        },
    );

    let tap = match tap {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("CGEventTap creation failed ({:?}) — ensure Accessibility permission is granted", e);
            return;
        }
    };

    let loop_src = match tap.mach_port.create_runloop_source(0) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create CFRunLoop source: {:?}", e);
            return;
        }
    };

    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&loop_src, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    info!("CGEventTap active — key interception running");
    CFRunLoop::run_current();
}

fn handle_event(
    detector:      &mut ChordDetector,
    ignore_next_c: &mut bool,
    ignore_next_x: &mut bool,
    tx:            &SyncSender<HotkeyAction>,
    event_type:    CGEventType,
    event:         &CGEvent,
    mode_static:   bool,
) -> Option<CGEvent> {
    let flags = event.get_flags();
    let cmd   = flags.contains(CGEventFlags::CGEventFlagCommand);
    let opt   = flags.contains(CGEventFlags::CGEventFlagAlternate);

    match event_type {

        // ── FlagsChanged: modifier press / release ────────────────────
        CGEventType::FlagsChanged => {
            sync_modifiers(detector, flags);
            debug!("Modifiers → cmd={} opt={}", cmd, opt);
            Some(event.to_owned())
        }

        // ── KeyDown ───────────────────────────────────────────────────
        CGEventType::KeyDown => {
            // Filter autorepeat
            let is_repeat = event
                .get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT)
                == 1;
            if is_repeat && detector.cmd() && detector.opt() {
                return None;
            }

            let keycode = event
                .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                as u16;
            let key = match keycode_to_rdev(keycode) {
                Some(k) => k,
                None    => return Some(event.to_owned()),
            };

            // Injection passthrough guard
            if key == rdev::Key::KeyC && *ignore_next_c {
                *ignore_next_c = false;
                debug!("Passing injected Cmd+C through");
                return Some(event.to_owned());
            }
            if key == rdev::Key::KeyX && *ignore_next_x {
                *ignore_next_x = false;
                debug!("Passing injected Cmd+X through");
                return Some(event.to_owned());
            }

            // Update detector held set FIRST so digit actions see C/X/V
            detector.key_down(key.clone());
            debug!("KeyDown {:?}  cmd={} opt={}", key, cmd, opt);

            if detector.cmd() && detector.opt() {
                // Set injection guard before firing so injected key passes through
                if key == rdev::Key::KeyC { *ignore_next_c = true; }
                if key == rdev::Key::KeyX { *ignore_next_x = true; }

                if let Some(action) = detector.evaluate_press(&key, mode_static) {
                    debug!("Action (press): {:?}", action);
                    let _ = tx.send(action);
                }
                return None; // suppress
            }

            Some(event.to_owned())
        }

        // ── KeyUp ─────────────────────────────────────────────────────
        // IMPORTANT: Use detector's OWN modifier state, not live flags.
        // Live flags can already show Opt=false by the time C KeyUp fires.
        // Additionally, even if was_active is false because a modifier
        // already fired a FlagsChanged event, we must still call
        // evaluate_release for any key that was pressed inside the chord
        // (tracked in keys_pressed_in_chord).  This closes the race where
        // Opt's FlagsChanged clears the detector before C's KeyUp arrives.
        CGEventType::KeyUp => {
            let keycode = event
                .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                as u16;
            let key = match keycode_to_rdev(keycode) {
                Some(k) => k,
                None    => return Some(event.to_owned()),
            };

            // was_active: both modifiers still held in detector at KeyUp time.
            let was_active = detector.cmd() && detector.opt();
            // pending: key was pressed while chord was active (may survive
            // modifier release via keys_pressed_in_chord).
            let is_pending = detector.is_pending_chord_key(&key);
            debug!("KeyUp {:?}  detector_active={}  pending={}", key, was_active, is_pending);

            // Fire evaluate_release if the chord was active OR if this key
            // was registered in the chord (handles modifier-first-release race).
            if was_active || is_pending {
                if let Some(action) = detector.evaluate_release(&key, mode_static) {
                    debug!("Action (release): {:?}", action);
                    let _ = tx.send(action);
                    detector.key_up(key);
                    return None; // suppress the raw key event
                }
            }

            detector.key_up(key);

            // Suppress raw event only if chord was active (not for pending-only case
            // where modifiers were already fully released before this KeyUp).
            if was_active { return None; }
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
        0  => KeyA,  1  => KeyS,  2  => KeyD,  3  => KeyF,
        4  => KeyH,  5  => KeyG,  6  => KeyZ,  7  => KeyX,
        8  => KeyC,  9  => KeyV,  11 => KeyB,  12 => KeyQ,
        13 => KeyW,  14 => KeyE,  15 => KeyR,  16 => KeyY,
        17 => KeyT,  31 => KeyO,  32 => KeyU,  34 => KeyI,
        35 => KeyP,  37 => KeyL,  38 => KeyJ,  40 => KeyK,
        45 => KeyN,  46 => KeyM,

        18 => Num1,  19 => Num2,  20 => Num3,
        21 => Num4,  23 => Num5,  22 => Num6,
        26 => Num7,  28 => Num8,  25 => Num9,
        29 => Num0,

        48  => Tab,
        53  => Escape,
        36  => Return,
        51  => Backspace,
        49  => Space,
        123 => LeftArrow, 124 => RightArrow,
        125 => DownArrow,  126 => UpArrow,
        _ => return None,
    })
}