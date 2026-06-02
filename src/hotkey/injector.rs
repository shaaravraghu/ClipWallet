//! Synthetic keystroke injection (Cmd+C / Cmd+X / Cmd+V).
//!
//! Every event injected here is built through CoreGraphics so we can stamp it
//! with [`SYNTHETIC_TAG`] in the `EVENT_SOURCE_USER_DATA` field. The event tap
//! reads that field first and passes tagged events straight through, which is
//! what lets us inject a paste/copy/cut without (a) the tap re-interpreting it
//! as a user chord or (b) the injection flipping the Idle/Active tier
//! mid-operation. Critically, ALL THREE paths apply the tag identically — a tag
//! missing from any one of them would corrupt tier state during that operation
//! (issue #27, review pt 6).
//!
//! We synthesize the chord by setting the Command flag directly on the letter
//! key event (rather than posting separate modifier key events), so each
//! injection is just a tagged KeyDown + KeyUp and produces no extra
//! FlagsChanged churn.

use crate::hotkey::SYNTHETIC_TAG;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::thread::sleep;
use std::time::Duration;
use tracing::error;

// macOS virtual keycodes (layout-independent).
const KEYCODE_C: CGKeyCode = 8;
const KEYCODE_X: CGKeyCode = 7;
const KEYCODE_V: CGKeyCode = 9;

const KEY_EVENT_GAP_MS: u64 = 20;
/// Time to let the foreground app process a copy/cut before the engine reads
/// the pasteboard.
const CLIPBOARD_SETTLE_MS: u64 = 150;
/// Delay before a dynamic paste so the user's physical modifiers are released
/// and the foreground app is ready to receive the keystroke.
const PASTE_PREDELAY_MS: u64 = 350;

/// Build one tagged keyboard event carrying the Command flag. A fresh event
/// source is created per event so we never depend on `CGEventSource` being
/// cloneable across crate versions.
fn tagged_cmd_event(keycode: CGKeyCode, key_down: bool) -> Option<CGEvent> {
    let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        Ok(s)  => s,
        Err(_) => {
            error!("injector: failed to create CGEventSource");
            return None;
        }
    };
    let event = match CGEvent::new_keyboard_event(source, keycode, key_down) {
        Ok(e)  => e,
        Err(_) => {
            error!("injector: failed to create keyboard event for keycode {}", keycode);
            return None;
        }
    };
    event.set_flags(CGEventFlags::CGEventFlagCommand);
    event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, SYNTHETIC_TAG);
    Some(event)
}

/// Post a tagged Cmd+<key> chord (down, gap, up) at the HID tap point.
fn post_tagged_cmd_chord(keycode: CGKeyCode) {
    if let Some(down) = tagged_cmd_event(keycode, true) {
        down.post(CGEventTapLocation::HID);
    }
    sleep(Duration::from_millis(KEY_EVENT_GAP_MS));
    if let Some(up) = tagged_cmd_event(keycode, false) {
        up.post(CGEventTapLocation::HID);
    }
}

/// Inject a tagged Cmd+C, then wait for the foreground app to service it.
pub fn simulate_copy() {
    post_tagged_cmd_chord(KEYCODE_C);
    sleep(Duration::from_millis(CLIPBOARD_SETTLE_MS));
}

/// Inject a tagged Cmd+X, then wait for the foreground app to service it.
pub fn simulate_cut() {
    post_tagged_cmd_chord(KEYCODE_X);
    sleep(Duration::from_millis(CLIPBOARD_SETTLE_MS));
}

/// Inject a tagged Cmd+V.
pub fn simulate_paste() {
    post_tagged_cmd_chord(KEYCODE_V);
}

/// Inject a tagged Cmd+V after a short delay, off-thread, so the user's
/// physical Cmd/Opt are fully released first. Used by dynamic paste.
pub fn simulate_paste_delayed() {
    std::thread::spawn(|| {
        sleep(Duration::from_millis(PASTE_PREDELAY_MS));
        simulate_paste();
    });
}
