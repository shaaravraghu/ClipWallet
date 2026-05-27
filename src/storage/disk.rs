use crate::clipboard::types::ClipEntry;
use crate::storage::ram::RamStore;
use dirs::home_dir;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};

pub fn store_dir() -> anyhow::Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("No home dir found"))?;
    Ok(home.join(".clipwallet").join("store"))
}

/// Atomically write bytes to path.
/// Writes to a .tmp file first, then renames — prevents corruption on crash.
fn atomic_write(path: &PathBuf, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Remove any leftover .tmp files from a previous crashed write.
/// Called once on startup before load().
pub fn cleanup_tmp_files() {
    let dir = match store_dir() {
        Ok(d) => d,
        Err(_) => return,
    };
    if !dir.exists() { return; }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tmp") {
                let _ = fs::remove_file(&path);
                tracing::warn!("Cleaned up orphaned tmp file: {:?}", path);
            }
        }
    }
}

/// Flush RAM → disk. Only writes if dirty flag is set.
pub fn flush(ram: &mut RamStore) -> anyhow::Result<()> {
    if !ram.is_dirty() {
        return Ok(());
    }

    let dir = store_dir()?;
    fs::create_dir_all(&dir)?;

    // ── Static slots ──────────────────────────────────────────────────
    for (i, slot) in ram.static_slots.iter().enumerate() {
        let path = dir.join(format!("slot_{}.mpk", i + 1));
        match slot {
            Some(entry) => {
                let bytes = rmp_serde::to_vec(entry)?;
                atomic_write(&path, &bytes)?;
            }
            None => { let _ = fs::remove_file(&path); }
        }
    }

    // ── Dynamic ring ──────────────────────────────────────────────────
    // Stored as Vec so order (recency) is preserved on reload
    let entries: Vec<&ClipEntry> = ram.dynamic_ring.iter().collect();
    let bytes = rmp_serde::to_vec(&entries)?;
    let ring_path = dir.join("dynamic_ring.mpk");
    atomic_write(&ring_path, &bytes)?;

    ram.clear_dirty();
    info!(
        "Flushed to disk — {} static slots, {} dynamic entries",
        ram.static_slots.iter().filter(|s| s.is_some()).count(),
        ram.dynamic_ring.len()
    );
    Ok(())
}

/// Load disk → RAM on startup. Preserves recency order via timestamps.
pub fn load(ram: &mut RamStore) -> anyhow::Result<()> {
    let dir = match store_dir() {
        Ok(d) => d,
        Err(e) => {
            warn!("Cannot determine store dir: {}", e);
            return Ok(());
        }
    };
    if !dir.exists() {
        info!("No existing store — starting fresh");
        return Ok(());
    }

    // ── Static slots ──────────────────────────────────────────────────
    let mut loaded_slots = 0usize;
    for i in 0..9usize {
        let path = dir.join(format!("slot_{}.mpk", i + 1));
        if !path.exists() { continue; }
        match fs::read(&path).and_then(|b| Ok(b)) {
            Ok(bytes) => match rmp_serde::from_slice::<ClipEntry>(&bytes) {
                Ok(entry) => {
                    ram.static_slots[i] = Some(entry);
                    loaded_slots += 1;
                }
                Err(e) => {
                    warn!("Corrupt slot_{}.mpk — skipping ({})", i + 1, e);
                    let _ = fs::remove_file(&path);
                }
            },
            Err(e) => error!("Cannot read slot_{}.mpk: {}", i + 1, e),
        }
    }

    // ── Dynamic ring ──────────────────────────────────────────────────
    let ring_path = dir.join("dynamic_ring.mpk");
    let mut loaded_dynamic = 0usize;
    if ring_path.exists() {
        match fs::read(&ring_path) {
            Ok(bytes) => match rmp_serde::from_slice::<Vec<ClipEntry>>(&bytes) {
                Ok(mut entries) => {
                    // Sort by timestamp descending so index 0 = most recent
                    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    loaded_dynamic = entries.len();
                    ram.dynamic_ring.extend(entries);
                }
                Err(e) => {
                    warn!("Corrupt dynamic_ring.mpk — starting fresh ring ({})", e);
                    let _ = fs::remove_file(&ring_path);
                }
            },
            Err(e) => error!("Cannot read dynamic_ring.mpk: {}", e),
        }
    }

    info!(
        "Loaded from disk — {} static slots, {} dynamic entries",
        loaded_slots, loaded_dynamic
    );
    Ok(())
}