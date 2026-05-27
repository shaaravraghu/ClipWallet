//! Global key interception.
//! macOS: CGEventTap — intercepts and suppresses key events before they reach apps.
//! Linux: rdev::grab  — global key grab via X11 Record extension.

use crate::hotkey::{ChordDetector, HotkeyAction};
use std::sync::mpsc::SyncSender;
use tracing::{debug, info};

// ═══════════════════════════════════════════════════════════════════════════════
// macOS — CGEventTap
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use core_foundation::base::TCFType;
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation,
        CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField,
    };
    use std::cell::RefCell;

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
        )
        .expect("CGEventTap creation failed — ensure Accessibility permission is granted");

        let loop_src = tap
            .mach_port
            .create_runloop_source(0)
            .expect("Failed to create CFRunLoop source");

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
            CGEventType::FlagsChanged => {
                sync_modifiers(detector, flags);
                debug!("Modifiers → cmd={} opt={}", cmd, opt);
                Some(event.to_owned())
            }

            CGEventType::KeyDown => {
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

                detector.key_down(key.clone());
                debug!("KeyDown {:?}  cmd={} opt={}", key, cmd, opt);

                if detector.cmd() && detector.opt() {
                    if key == rdev::Key::KeyC { *ignore_next_c = true; }
                    if key == rdev::Key::KeyX { *ignore_next_x = true; }

                    if let Some(action) = detector.evaluate_press(&key, mode_static) {
                        debug!("Action (press): {:?}", action);
                        let _ = tx.send(action);
                    }
                    return None;
                }

                Some(event.to_owned())
            }

            CGEventType::KeyUp => {
                let keycode = event
                    .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                    as u16;
                let key = match keycode_to_rdev(keycode) {
                    Some(k) => k,
                    None    => return Some(event.to_owned()),
                };

                let was_active = detector.cmd() && detector.opt();
                let is_pending = detector.is_pending_chord_key(&key);
                debug!("KeyUp {:?}  detector_active={}  pending={}", key, was_active, is_pending);

                if was_active || is_pending {
                    if let Some(action) = detector.evaluate_release(&key, mode_static) {
                        debug!("Action (release): {:?}", action);
                        let _ = tx.send(action);
                        detector.key_up(key);
                        return None;
                    }
                }

                detector.key_up(key);

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
}

// ═══════════════════════════════════════════════════════════════════════════════
// Linux — rdev::listen
// ═══════════════════════════════════════════════════════════════════════════════
// Note: listen() cannot suppress events — hotkey keypresses pass through to
// the active app. For a full intercept, install the evdev system library and
// enable the "unstable_grab" feature on the rdev crate.

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;
    use rdev::{listen, Event, EventType};

    pub fn start_event_tap(tx: SyncSender<HotkeyAction>, mode_static: bool) {
        let mut detector = ChordDetector::new();
        info!("rdev listen active — key monitoring running");

        if let Err(e) = listen(move |event: Event| {
            handle_event(&mut detector, &tx, event, mode_static)
        }) {
            tracing::error!("rdev::listen failed: {:?}", e);
        }
    }

    fn handle_event(
        detector:    &mut ChordDetector,
        tx:          &SyncSender<HotkeyAction>,
        event:       Event,
        mode_static: bool,
    ) {
        match event.event_type {
            EventType::KeyPress(key) => {
                detector.key_down(key.clone());
                debug!("KeyPress {:?}", key);

                if detector.cmd() && detector.opt() {
                    if let Some(action) = detector.evaluate_press(&key, mode_static) {
                        debug!("Action (press): {:?}", action);
                        let _ = tx.send(action);
                    }
                }
            }

            EventType::KeyRelease(key) => {
                let was_active = detector.cmd() && detector.opt();
                let is_pending = detector.is_pending_chord_key(&key);
                debug!("KeyRelease {:?}  active={}  pending={}", key, was_active, is_pending);

                if was_active || is_pending {
                    if let Some(action) = detector.evaluate_release(&key, mode_static) {
                        debug!("Action (release): {:?}", action);
                        let _ = tx.send(action);
                        detector.key_up(key);
                        return;
                    }
                }

                detector.key_up(key);
            }

            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════════

pub use platform::start_event_tap;