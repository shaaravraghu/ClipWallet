use crate::clipboard::mime::{detect_bytes, normalise_image, DetectedType};
use crate::clipboard::pasteboard;
use crate::clipboard::types::{ClipData, ClipEntry};
use crate::hotkey::{
    simulate_copy, simulate_cut, simulate_paste, simulate_paste_delayed, HotkeyAction,
};
use crate::notify;
use crate::static_store::StaticSlotStore;
use crate::storage::encrypted::{load_from_vault, save_to_vault};
use crate::storage::ram::{next_id, PushResult, RamStore};
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

const PASTE_SETTLE_MS: u64 = 60;

pub struct Engine {
    ram:          Arc<RwLock<RamStore>>,
    clipboard:    Clipboard,
    static_store: StaticSlotStore,
}

impl Engine {
    pub fn new(ram: Arc<RwLock<RamStore>>) -> anyhow::Result<Self> {
        Ok(Self {
            ram,
            clipboard:    Clipboard::new()?,
            static_store: StaticSlotStore::new(),
        })
    }

    pub fn handle(&mut self, action: HotkeyAction) {
        info!("[ACTION] {:?}", action);
        match action {
            HotkeyAction::StaticCopy(s)       => self.static_copy(s),
            HotkeyAction::StaticCut(s)        => self.static_cut(s),
            HotkeyAction::StaticPaste(s)      => self.static_paste(s),
            HotkeyAction::StaticNavigateNext  => self.static_nav(1),
            HotkeyAction::StaticNavigatePrev  => self.static_nav(-1),
            HotkeyAction::StaticDeleteCurrent => self.static_delete(),
            HotkeyAction::StaticTogglePin(s)  => self.static_toggle_pin(s),
            HotkeyAction::DynamicCopy         => self.dynamic_copy(),
            HotkeyAction::DynamicCut          => self.dynamic_cut(),
            HotkeyAction::DynamicPaste        => self.dynamic_paste(),
            HotkeyAction::DynamicNavigateNext => self.dynamic_nav(1),
            HotkeyAction::DynamicNavigatePrev => self.dynamic_nav(-1),
            HotkeyAction::DynamicDeleteCurrent=> self.dynamic_delete(),
            HotkeyAction::DynamicTogglePin    => self.dynamic_toggle_pin(),
            HotkeyAction::CursorReset         => self.cursor_reset(),
        }
    }

    // ── Clipboard read ────────────────────────────────────────────────

    fn read_clipboard(&mut self) -> Option<ClipData> {
        if let Some(paths) = pasteboard::read_file_paths() {
            info!("Read {} file path(s)", paths.len());
            return Some(ClipData::FilePath(paths));
        }
        if let Some(rtf) = pasteboard::read_rtf() {
            info!("Read RTF ({} bytes)", rtf.len());
            return Some(ClipData::RichText(rtf));
        }
        if let Ok(img) = self.clipboard.get_image() {
            let raw: Vec<u8> = img.bytes.into_owned();
            match normalise_image(&raw) {
                Ok(png) => {
                    info!("Read Image → PNG ({} bytes)", png.len());
                    return Some(ClipData::Image {
                        bytes: png, width: img.width, height: img.height,
                    });
                }
                Err(e) => warn!("Image normalise failed: {}", e),
            }
            return Some(ClipData::Binary(raw));
        }
        if let Ok(text) = self.clipboard.get_text() {
            if !text.is_empty() {
                info!("Read PlainText ({} chars)", text.len());
                return Some(ClipData::PlainText(text));
            }
        }
        warn!("Clipboard empty or unreadable");
        None
    }

    // ── Sync to system clipboard ──────────────────────────────────────

    fn sync_to_system_clipboard(&mut self, data: &ClipData) -> bool {
        match data {
            ClipData::PlainText(text) => {
                match self.clipboard.set_text(text.clone()) {
                    Ok(_)  => { debug!("Synced PlainText ({} chars)", text.len()); true }
                    Err(e) => { warn!("Clipboard sync failed: {}", e); false }
                }
            }
            ClipData::RichText(bytes) => {
                let ok = pasteboard::write_rtf(bytes);
                if !ok { warn!("RTF sync failed"); }
                ok
            }
            ClipData::Image { bytes, width, height } => {
                let img = ImageData {
                    bytes: Cow::Borrowed(bytes), width: *width, height: *height,
                };
                match self.clipboard.set_image(img) {
                    Ok(_)  => true,
                    Err(e) => { warn!("Image sync failed: {}", e); false }
                }
            }
            ClipData::FilePath(paths) => {
                let ok = pasteboard::write_file_paths(paths);
                if !ok { warn!("FilePath sync failed"); }
                ok
            }
            ClipData::Binary(bytes) => {
                match detect_bytes(bytes) {
                    DetectedType::PlainText => {
                        let t = String::from_utf8_lossy(bytes).to_string();
                        self.clipboard.set_text(t).is_ok()
                    }
                    _ => { warn!("Binary cannot be synced"); false }
                }
            }
        }
    }

    fn write_and_paste(&mut self, data: &ClipData) {
        if self.sync_to_system_clipboard(data) {
            std::thread::sleep(std::time::Duration::from_millis(PASTE_SETTLE_MS));
            simulate_paste();
        }
    }

    // ── Short preview for notifications ───────────────────────────────

    fn short_preview(data: &ClipData) -> String {
        match data {
            ClipData::PlainText(t) => {
                let s: String = t.chars().take(40).collect();
                let s = s.replace('\n', " ");
                if t.len() > 40 { format!("{}…", s) } else { s }
            }
            ClipData::Image { width, height, .. } => format!("Image {}×{}", width, height),
            ClipData::FilePath(p) => format!("{} file(s)", p.len()),
            ClipData::RichText(b) => format!("RTF {} bytes", b.len()),
            ClipData::Binary(b)   => format!("Binary {} bytes", b.len()),
        }
    }

    // ── Debug logging ─────────────────────────────────────────────────

    fn entry_preview(&self, entry: &ClipEntry) -> String {
        match &entry.data {
            ClipData::PlainText(t) => {
                let s: String = t.chars().take(40).collect();
                let s = s.replace('\n', "↵");
                if t.len() > 40 { format!("\"{}…\"", s) } else { format!("\"{}\"", s) }
            }
            ClipData::Image { width, height, .. } => format!("[Image {}x{}]", width, height),
            ClipData::FilePath(p)  => format!("[{} file(s)]", p.len()),
            ClipData::RichText(b)  => format!("[RTF {} bytes]", b.len()),
            ClipData::Binary(b)    => format!("[Binary {} bytes]", b.len()),
        }
    }

    fn log_static_state(&self) {
        debug!("┌─ Static Slots ({}/{} occupied) ──────────────────────",
            self.static_store.occupied_count(), 9);
        for i in 0..9 {
            let marker = if i == self.static_store.cursor { "►" } else { " " };
            match &self.static_store.slots[i] {
                Some(e) => debug!("│ {} slot {} → id={} type={} | {}",
                    marker, i+1, e.id, e.data.type_label(), self.entry_preview(e)),
                None    => debug!("│ {} slot {} → NULL", marker, i+1),
            }
        }
        debug!("└──────────────────────────────────────────────────────");
    }

    fn log_ring_state(&self) {
        let ram = self.ram.read().unwrap();
        let len = ram.ring_len();
        let cur = ram.dynamic_cursor;
        debug!("┌─ Ring State ({} entries) ─────────────────────────", len);
        for (i, entry) in ram.dynamic_ring.iter().enumerate() {
            let marker = if i == cur { "►" } else { " " };
            debug!("│ {} [{}] id={} type={} | {}",
                marker, i, entry.id, entry.data.type_label(), self.entry_preview(entry));
        }
        if len == 0 { debug!("│   (empty)"); }
        debug!("└─ Cursor [{}] — live in Cmd+V ──────────────────────", cur);
    }

    // ════════════════════════════════════════════════════════════════
    // STATIC MODE
    // ════════════════════════════════════════════════════════════════

    fn static_copy(&mut self, slot: usize) {
        // Refuse if slot is pinned
        if self.static_store.read(slot).map(|e| e.pinned).unwrap_or(false) {
            warn!("[Static][COPY] slot={} is pinned — copy blocked", slot);
            notify::notify_static_pin_blocked(slot);
            return;
        }
        info!("[Static][COPY] slot={} — injecting Cmd+C", slot);
        simulate_copy();
        if let Some(data) = self.read_clipboard() {
            let preview = Self::short_preview(&data);
            let entry   = ClipEntry::new(next_id(), data);
            info!("[Static][COPY] slot={} ← id={} type={}", slot, entry.id, entry.data.type_label());
            self.static_store.write(slot, entry);
            self.log_static_state();
            notify::notify_static_copy(slot, &preview);
        }
    }

    fn static_cut(&mut self, slot: usize) {
        // Refuse if slot is pinned
        if self.static_store.read(slot).map(|e| e.pinned).unwrap_or(false) {
            warn!("[Static][CUT] slot={} is pinned — cut blocked", slot);
            notify::notify_static_pin_blocked(slot);
            return;
        }
        info!("[Static][CUT] slot={} — injecting Cmd+X", slot);
        simulate_cut();
        if let Some(data) = self.read_clipboard() {
            let preview = Self::short_preview(&data);
            let entry   = ClipEntry::new(next_id(), data);
            info!("[Static][CUT] slot={} ← id={} type={}", slot, entry.id, entry.data.type_label());
            self.static_store.write(slot, entry);
            self.log_static_state();
            notify::notify_static_cut(slot, &preview);
        }
    }

    fn static_paste(&mut self, slot: usize) {
        let data = self.static_store.read(slot).map(|e| e.data.clone());
        match data {
            Some(d) => {
                info!("[Static][PASTE] slot={} — syncing + scheduling", slot);
                self.sync_to_system_clipboard(&d);
                simulate_paste_delayed();
                notify::notify_static_paste(slot);
            }
            None => warn!("[Static][PASTE] slot={} is NULL", slot),
        }
    }

    fn static_nav(&mut self, direction: i32) {
        let result = if direction > 0 {
            self.static_store.cursor_next()
        } else {
            self.static_store.cursor_prev()
        };
        match result {
            Some(slot) => {
                let data = self.static_store.cursor_entry()
                    .map(|e| (e.id, e.data.type_label(), e.data.clone()));
                if let Some((id, type_label, d)) = data {
                    let preview = Self::short_preview(&d);
                    info!("[Static][NAV] → slot {} id={} type={}", slot, id, type_label);
                    self.sync_to_system_clipboard(&d);
                    self.log_static_state();
                    notify::notify_static_nav(slot, &preview);
                }
            }
            None => warn!("[Static][NAV] All slots NULL"),
        }
    }

    fn static_delete(&mut self) {
        let slot     = self.static_store.cursor_slot();
        let was_some = self.static_store.cursor_entry().is_some();
        if was_some {
            self.static_store.delete_at_cursor();
            info!("[Static][DELETE] slot={} → NULL", slot);
            notify::notify_static_delete(slot);
        } else {
            warn!("[Static][DELETE] slot={} already NULL", slot);
        }
        self.log_static_state();
    }

    // ════════════════════════════════════════════════════════════════
    // DYNAMIC MODE
    // ════════════════════════════════════════════════════════════════

    fn dynamic_copy(&mut self) {
        info!("[Dynamic][COPY] Injecting Cmd+C...");
        simulate_copy();
        if let Some(data) = self.read_clipboard() {
            let preview = Self::short_preview(&data);
            let entry   = ClipEntry::new(next_id(), data);
            info!("[Dynamic][COPY] id={} type={} size={}B → ring[0]",
                entry.id, entry.data.type_label(), entry.data.size_bytes());
            match self.ram.write().unwrap().push_dynamic(entry.clone()) {
                PushResult::Evicted(old) => info!("[Dynamic][EVICT] Dropped: {}", old),
                PushResult::AllPinned    => {
                    warn!("[Dynamic][COPY] Ring full and all entries pinned — push blocked");
                    return;
                }
                PushResult::Pushed => {}
            }
            self.sync_to_system_clipboard(&entry.data);
            let (pos, total) = {
                let ram = self.ram.read().unwrap();
                (ram.dynamic_cursor + 1, ram.ring_len())
            };
            self.log_ring_state();
            notify::notify_dynamic_copy(pos, total, &preview);
        }
    }

    fn dynamic_cut(&mut self) {
        info!("[Dynamic][CUT] Injecting Cmd+X...");
        simulate_cut();
        if let Some(data) = self.read_clipboard() {
            let preview = Self::short_preview(&data);
            let entry   = ClipEntry::new(next_id(), data);
            info!("[Dynamic][CUT] id={} type={} → ring[0]", entry.id, entry.data.type_label());
            match self.ram.write().unwrap().push_dynamic(entry.clone()) {
                PushResult::Evicted(old) => info!("[Dynamic][EVICT] Dropped: {}", old),
                PushResult::AllPinned    => {
                    warn!("[Dynamic][CUT] Ring full and all entries pinned — push blocked");
                    return;
                }
                PushResult::Pushed => {}
            }
            self.sync_to_system_clipboard(&entry.data);
            let (pos, total) = {
                let ram = self.ram.read().unwrap();
                (ram.dynamic_cursor + 1, ram.ring_len())
            };
            self.log_ring_state();
            notify::notify_dynamic_cut(pos, total, &preview);
        }
    }

    fn dynamic_paste(&mut self) {
        let data = {
            let ram = self.ram.read().unwrap();
            ram.current_dynamic().map(|e| (e.id, e.data.clone(), ram.dynamic_cursor, ram.ring_len()))
        };
        match data {
            Some((id, d, cursor, total)) => {
                info!("[Dynamic][PASTE] ring[{}] id={} — syncing + scheduling", cursor, id);
                self.sync_to_system_clipboard(&d);
                simulate_paste_delayed();
                notify::notify_dynamic_paste(cursor + 1, total);
            }
            None => warn!("[Dynamic][PASTE] Ring is empty"),
        }
    }

    fn dynamic_nav(&mut self, direction: i32) {
        {
            let mut ram = self.ram.write().unwrap();
            if direction > 0 { ram.cursor_next(); } else { ram.cursor_prev(); }
        }
        let data = {
            let ram = self.ram.read().unwrap();
            ram.current_dynamic().map(|e| {
                (e.id, e.data.type_label(), ram.dynamic_cursor, ram.ring_len(), e.data.clone())
            })
        };
        if let Some((id, type_label, cursor, total, d)) = data {
            let preview = Self::short_preview(&d);
            info!("[Dynamic][NAV] [{}/{}] id={} type={}", cursor+1, total, id, type_label);
            self.sync_to_system_clipboard(&d);
            self.log_ring_state();
            notify::notify_dynamic_nav(cursor + 1, total, &preview);
        }
    }

    fn dynamic_delete(&mut self) {
        let cursor = self.ram.read().unwrap().dynamic_cursor;
        let id     = self.ram.read().unwrap().current_dynamic().map(|e| e.id);
        self.ram.write().unwrap().delete_at_cursor();
        info!("[Dynamic][DELETE] Removed ring[{}] id={:?}", cursor, id);
        let next = self.ram.read().unwrap().current_dynamic().map(|e| e.data.clone());
        if let Some(d) = next { self.sync_to_system_clipboard(&d); }
        self.log_ring_state();
        notify::notify_dynamic_delete(cursor);
    }

    fn dynamic_toggle_pin(&mut self) {
        let result = {
            let mut ram = self.ram.write().unwrap();
            let cursor  = ram.dynamic_cursor;
            ram.toggle_pin_at_cursor().map(|pinned| (cursor, pinned))
        };
        match result {
            Some((cursor, true))  => {
                info!("[Dynamic][PIN] ring[{}] pinned", cursor);
                notify::notify_dynamic_pin(cursor, true);
            }
            Some((cursor, false)) => {
                info!("[Dynamic][PIN] ring[{}] unpinned", cursor);
                notify::notify_dynamic_pin(cursor, false);
            }
            None => warn!("[Dynamic][PIN] Ring is empty"),
        }
    }

    fn static_toggle_pin(&mut self, slot: usize) {
        if let Some(entry) = self.static_store.slots.get_mut(slot - 1).and_then(|s| s.as_mut()) {
            entry.pinned = !entry.pinned;
            let pinned = entry.pinned;
            info!("[Static][PIN] slot={} pinned={}", slot, pinned);
            notify::notify_static_pin(slot, pinned);
        } else {
            warn!("[Static][PIN] slot={} is NULL", slot);
        }
    }

    fn cursor_reset(&mut self) {
        let was_at = self.ram.read().unwrap().dynamic_cursor;
        if was_at != 0 {
            self.ram.write().unwrap().reset_cursor();
            info!("[Dynamic][TIMEOUT] Cursor reset [{}]→[0]", was_at + 1);
            let d = self.ram.read().unwrap().current_dynamic().map(|e| e.data.clone());
            if let Some(data) = d { self.sync_to_system_clipboard(&data); }
            self.log_ring_state();
        }
    }

    // ── Vault ─────────────────────────────────────────────────────────

    pub fn encrypt_slot(&mut self, slot: usize) {
        let entry = self.static_store.read(slot).cloned();
        match entry {
            Some(mut e) => {
                e.encrypted = true;
                match save_to_vault(&e) {
                    Ok(path) => {
                        self.static_store.clear(slot);
                        info!("[Vault] Encrypted slot {} → {:?}", slot, path);
                    }
                    Err(e) => warn!("[Vault] Encrypt failed: {}", e),
                }
            }
            None => warn!("[Vault] Slot {} is NULL", slot),
        }
    }

    pub fn decrypt_slot(&mut self, id: u64, slot: usize) {
        match load_from_vault(id) {
            Ok(mut entry) => {
                entry.encrypted = false;
                info!("[Vault] Decrypted id={} → slot {}", id, slot);
                self.static_store.write(slot, entry);
            }
            Err(e) => warn!("[Vault] Decrypt failed: {}", e),
        }
    }
}