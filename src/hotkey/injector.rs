use rdev::{simulate, EventType, Key};
use std::thread::sleep;
use std::time::Duration;

// Platform modifier key for clipboard shortcuts.
// macOS: Cmd (MetaLeft), Linux: Ctrl (ControlLeft)
#[cfg(target_os = "macos")]
const MOD_KEY: Key = Key::MetaLeft;
#[cfg(not(target_os = "macos"))]
const MOD_KEY: Key = Key::ControlLeft;

fn press(key: Key) {
    let _ = simulate(&EventType::KeyPress(key.clone()));
    sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyRelease(key));
}

fn release_opt() {
    let _ = simulate(&EventType::KeyRelease(Key::Alt));
    let _ = simulate(&EventType::KeyRelease(Key::AltGr));
}

/// Inject Cmd+C / Ctrl+C cleanly.
/// Releases Opt first so our tap won't re-catch the chord.
pub fn simulate_copy() {
    release_opt();
    sleep(Duration::from_millis(30));
    let _ = simulate(&EventType::KeyPress(MOD_KEY));
    sleep(Duration::from_millis(20));
    press(Key::KeyC);
    sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyRelease(MOD_KEY));
    sleep(Duration::from_millis(150));
}

/// Inject Cmd+X / Ctrl+X cleanly.
pub fn simulate_cut() {
    release_opt();
    sleep(Duration::from_millis(30));
    let _ = simulate(&EventType::KeyPress(MOD_KEY));
    sleep(Duration::from_millis(20));
    press(Key::KeyX);
    sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyRelease(MOD_KEY));
    sleep(Duration::from_millis(150));
}

/// Inject Cmd+V / Ctrl+V.
pub fn simulate_paste() {
    sleep(Duration::from_millis(50));
    let _ = simulate(&EventType::KeyPress(MOD_KEY));
    sleep(Duration::from_millis(20));
    press(Key::KeyV);
    sleep(Duration::from_millis(20));
    let _ = simulate(&EventType::KeyRelease(MOD_KEY));
}

/// Inject paste after a delay so modifiers are fully released first.
pub fn simulate_paste_delayed() {
    std::thread::spawn(|| {
        sleep(Duration::from_millis(350));
        simulate_paste();
    });
}