//! macOS CGEventTap — intercepts and suppresses key events.
//! Uses a two-tier check strategy:
//!   1. Short-circuits any synthetic event marked with SYNTHETIC_TAG.
//!   2. Intercepts user hotkeys and maps them to HotkeyActions.

use crate::hotkey::chord::tier_active;
use crate::hotkey::{ChordDetector, HotkeyAction, SYNTHETIC_TAG};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopRef, CFRunLoopStop};
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField,
};
use core_graphics::event_source::CGEventSourceStateID;
use tracing::{debug, info, warn};

extern "C" {
    /// Enable / disable an event tap by its mach port. Needed to re-arm the tap
    /// from inside its own callback.
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
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
        unsafe { CFRunLoopStop(self.run_loop.0); }
    }
}

/// Spawn the event tap on its own OS thread and return a stoppable handle.
pub fn spawn_event_tap(
    tx: SyncSender<HotkeyAction>,
    mode_static: bool,
) -> GrabberHandle {
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
    let detector = RefCell::new(ChordDetector::new());
    let suppressed_cb = suppressed.clone();

    let tap_port: Rc<Cell<Option<CFMachPortRef>>> = Rc::new(Cell::new(None));
    let tap_port_cb = Rc::clone(&tap_port);

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
        move |_proxy, event_type, event| {
            if suppressed_cb.load(Ordering::Relaxed) {
                return Some(event.to_owned());
            }

            match event_type {
                CGEventType::TapDisabledByTimeout
                | CGEventType::TapDisabledByUserInput => {
                    if let Some(port) = tap_port_cb.get() {
                        unsafe { CGEventTapEnable(port, true); }
                    }
                    warn!("Event tap re-enabled after timeout or user disable.");
                    None
                }
                _ => handle_event(
                    &mut detector.borrow_mut(),
                    &tx,
                    event_type,
                    event,
                    mode_static,
                ),
            }
        },
    )
    .expect("CGEventTap creation failed — ensure Accessibility permission is granted");

    tap_port.set(Some(tap.mach_port.as_concrete_TypeRef()));

    let loop_src = tap
        .mach_port
        .create_runloop_source(0)
        .expect("Failed to create CFRunLoop source");

    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&loop_src, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    let _ = loop_tx.send(RunLoopHandle(run_loop.as_concrete_TypeRef()));
    
    info!("CGEventTap active — key interception running");
    CFRunLoop::run_current();
    info!("CGEventTap stopped");
}

fn handle_event(
    detector:    &mut ChordDetector,
    tx:          &SyncSender<HotkeyAction>,
    event_type:  CGEventType,
    event:       &CGEvent,
    mode_static: bool,
) -> Option<CGEvent> {
    // Synthetic-event short-circuit:
    // If it carries the SYNTHETIC_TAG user data, let it pass through immediately.
    if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == SYNTHETIC_TAG {
        return Some(event.to_owned());
    }

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

            detector.key_down(key.clone());
            debug!("KeyDown {:?}  cmd={} opt={}", key, cmd, opt);

            if detector.cmd() && detector.opt() {
                if let Some(action) = detector.evaluate_press(&key, mode_static) {
                    debug!("Action (press): {:?}", action);
                    let _ = tx.send(action);
                }
                return None; // suppress user chord keydown
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
                    return None; // suppress user chord keyup
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
