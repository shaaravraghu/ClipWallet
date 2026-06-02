//! macOS system notifications for ClipWallet.
//! Uses osascript — no extra crate, works on all macOS versions.
//! Spawned fire-and-forget so it never blocks the event loop.

use std::process::Command;

/// Send a native macOS banner notification.
pub fn notify(title: &str, subtitle: &str, body: &str) {
    let t = title.replace('"', "'");
    let s = subtitle.replace('"', "'");
    let b = body.replace('"', "'");

    let script = if s.is_empty() {
        format!(r#"display notification "{}" with title "{}""#, b, t)
    } else {
        format!(r#"display notification "{}" with title "{}" subtitle "{}""#, b, t, s)
    };

    let _ = Command::new("osascript").args(["-e", &script]).spawn();
}

// ── Static mode ───────────────────────────────────────────────────────────────

pub fn notify_static_copy(slot: usize, preview: &str) {
    notify("ClipWallet", &format!("Copied → Slot {}", slot), preview);
}

pub fn notify_static_cut(slot: usize, preview: &str) {
    notify("ClipWallet", &format!("Cut → Slot {}", slot), preview);
}

pub fn notify_static_paste(slot: usize) {
    notify("ClipWallet", &format!("Pasted from Slot {}", slot), "");
}

pub fn notify_static_nav(slot: usize, preview: &str) {
    notify("ClipWallet", &format!("Tab → Slot {}  (Cmd+V ready)", slot), preview);
}

pub fn notify_static_delete(slot: usize) {
    notify("ClipWallet", &format!("Slot {} cleared", slot), "");
}

// ── Dynamic mode ──────────────────────────────────────────────────────────────

pub fn notify_dynamic_copy(pos: usize, total: usize, preview: &str) {
    notify("ClipWallet", &format!("Copied  [{}/{}]", pos, total), preview);
}

pub fn notify_dynamic_cut(pos: usize, total: usize, preview: &str) {
    notify("ClipWallet", &format!("Cut  [{}/{}]", pos, total), preview);
}

pub fn notify_dynamic_paste(pos: usize, total: usize) {
    notify("ClipWallet", &format!("Paste ready  [{}/{}]", pos, total), "Press Cmd+V");
}

pub fn notify_dynamic_nav(pos: usize, total: usize, preview: &str) {
    notify("ClipWallet", &format!("Tab  [{}/{}]  Cmd+V ready", pos, total), preview);
}

pub fn notify_dynamic_delete(pos: usize) {
    notify("ClipWallet", &format!("Deleted ring[{}]", pos), "");
}

// ── Admin ─────────────────────────────────────────────────────────────────────

pub fn notify_mode_changed(mode: &str) {
    notify("ClipWallet", "Mode changed", &format!("Now in {} mode — restart to apply", mode));
}

pub fn notify_memory_cleared() {
    notify("ClipWallet", "Memory cleared", "All clipboard history erased");
}

pub fn notify_encryption_removed() {
    notify("ClipWallet", "Encryption removed", "Vault data deleted, Keychain key removed");
}