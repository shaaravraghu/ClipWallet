use crate::clipboard::types::ClipEntry;
use crate::storage::ram::RamStore;
use chrono::Utc;
use dirs::home_dir;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::{error, info, warn};

pub fn store_dir() -> PathBuf {
    home_dir()
        .expect("No home dir")
        .join(".clipwallet")
        .join("store")
}

// ─── Envelope format ────────────────────────────────────────────
// Layout:  [ "CW1\0" (4) | crc32 (4 LE) | entry_count (4 LE) | msgpack bytes ]
//
// Detects two failure modes MessagePack alone won't:
//   - truncated writes (entry_count mismatch after decode)
//   - partial-content writes that still happen to decode (crc mismatch)
//
// Read path is backward-compatible: files without the magic are decoded
// as bare MessagePack (the old format) and silently rewritten in the new
// format on next flush. No existing user loses their data on upgrade.

const ENVELOPE_MAGIC: &[u8; 4] = b"CW1\0";
const ENVELOPE_HEADER_LEN: usize = 12;

fn wrap_envelope(payload: &[u8], entry_count: u32) -> Vec<u8> {
    let crc = crc32fast::hash(payload);
    let mut out = Vec::with_capacity(ENVELOPE_HEADER_LEN + payload.len());
    out.extend_from_slice(ENVELOPE_MAGIC);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&entry_count.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

enum DecodeResult<T> {
    Validated(T),       // new format, crc + count verified
    LegacyUnvalidated(T), // old format, decoded but no integrity check
    Corrupt(String),
}

fn decode_envelope<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    expected_count: Option<usize>,
) -> DecodeResult<T> {
    if bytes.starts_with(ENVELOPE_MAGIC) {
        if bytes.len() < ENVELOPE_HEADER_LEN {
            return DecodeResult::Corrupt("envelope truncated (< header)".into());
        }
        let crc_stored = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let count_stored = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let payload = &bytes[ENVELOPE_HEADER_LEN..];

        let crc_computed = crc32fast::hash(payload);
        if crc_computed != crc_stored {
            return DecodeResult::Corrupt(format!(
                "checksum mismatch: stored=0x{:08x} computed=0x{:08x}",
                crc_stored, crc_computed
            ));
        }
        match rmp_serde::from_slice::<T>(payload) {
            Ok(value) => {
                if let Some(expected) = expected_count {
                    if expected != count_stored {
                        return DecodeResult::Corrupt(format!(
                            "envelope count {} != caller expected {}",
                            count_stored, expected
                        ));
                    }
                }
                DecodeResult::Validated(value)
            }
            Err(e) => DecodeResult::Corrupt(format!("decode failed: {}", e)),
        }
    } else {
        // Legacy bare-msgpack format. Decode and trust.
        match rmp_serde::from_slice::<T>(bytes) {
            Ok(value) => DecodeResult::LegacyUnvalidated(value),
            Err(e) => DecodeResult::Corrupt(format!("legacy decode failed: {}", e)),
        }
    }
}

/// Quarantine a corrupt file by renaming it rather than deleting.
/// Preserves it for post-mortem diagnostics.
fn quarantine(path: &PathBuf, reason: &str) {
    let ts = Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let dst = path.with_extension(format!("corrupt.{}", ts));
    match fs::rename(path, &dst) {
        Ok(_)  => warn!("Quarantined corrupt file → {:?} ({})", dst, reason),
        Err(e) => error!("Quarantine failed for {:?}: {} (original reason: {})", path, e, reason),
    }
}

// ─── Atomic durable write (Feedback #4) ───────────────────────────────────────
// Correct sequence: write tmp → fsync tmp → rename → fsync parent dir.
// The previous version was: write tmp → rename, which means the rename
// can be durable while the file's bytes are not — on next boot the target
// could be partial or zero-length, and only the new envelope crc would
// catch it. With fsync on the tmp file first, the bytes are guaranteed
// to be on stable storage before the directory entry flips.

fn atomic_write(path: &PathBuf, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new()
            .write(true).create(true).truncate(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;              // ← content reaches disk
    }
    fs::rename(&tmp, path)?;

    if let Some(parent) = path.parent() {
        // Best-effort: not all filesystems return Ok on directory fsync,
        // and it's not a correctness problem if it's not supported here.
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all(); // ← rename entry reaches disk
        }
    }
    Ok(())
}

pub fn cleanup_tmp_files() {
    let dir = store_dir();
    if !dir.exists() { return; }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tmp") {
                let _ = fs::remove_file(&path);
                warn!("Cleaned up orphaned tmp file: {:?}", path);
            }
        }
    }
}

// ─── Flush ────────────────────────────────────────────────────────────────────

pub fn flush(ram: &mut RamStore) -> anyhow::Result<()> {
    if !ram.is_dirty() {
        return Ok(());
    }
    flush_force(ram)
}

/// Flush regardless of the dirty flag. Used by the shutdown handler — we
/// don't trust the flag across panics or partial mutations.
pub fn flush_force(ram: &mut RamStore) -> anyhow::Result<()> {
    let dir = store_dir();
    fs::create_dir_all(&dir)?;

    // Static slots: one file per slot, each wraps a single ClipEntry.
    for (i, slot) in ram.static_slots.iter().enumerate() {
        let path = dir.join(format!("slot_{}.mpk", i + 1));
        match slot {
            Some(entry) => {
                let payload = rmp_serde::to_vec(entry)?;
                let bytes = wrap_envelope(&payload, 1);
                atomic_write(&path, &bytes)?;
            }
            None => { let _ = fs::remove_file(&path); }
        }
    }

    // Dynamic ring: single file, entire Vec.
    let entries: Vec<&ClipEntry> = ram.dynamic_ring.iter().collect();
    let payload = rmp_serde::to_vec(&entries)?;
    let bytes = wrap_envelope(&payload, entries.len() as u32);
    let ring_path = dir.join("dynamic_ring.mpk");
    atomic_write(&ring_path, &bytes)?;

    ram.clear_dirty();

    // Reclaim blob files orphaned since the last flush (ring evictions, slot
    // overwrites, deletions, vault encryption). A mutation always sets the dirty
    // flag, so every orphan is collected within one flush interval. See
    // storage::spill for the lifecycle rationale.
    crate::storage::spill::gc_orphans(ram);

    info!(
        "Flushed to disk — {} static slots, {} dynamic entries",
        ram.static_slots.iter().filter(|s| s.is_some()).count(),
        ram.dynamic_ring.len()
    );
    Ok(())
}

// ─── Load ─────────────────────────────────────────────────────────────────────

pub fn load(ram: &mut RamStore) -> anyhow::Result<()> {
    let dir = store_dir();
    if !dir.exists() {
        info!("No existing store — starting fresh");
        return Ok(());
    }

    let mut needs_rewrite = false;
    let mut loaded_slots = 0usize;

    for i in 0..9usize {
        let path = dir.join(format!("slot_{}.mpk", i + 1));
        if !path.exists() { continue; }
        match fs::read(&path) {
            Ok(bytes) => match decode_envelope::<ClipEntry>(&bytes, Some(1)) {
                DecodeResult::Validated(entry) => {
                    ram.static_slots[i] = Some(entry);
                    loaded_slots += 1;
                }
                DecodeResult::LegacyUnvalidated(entry) => {
                    ram.static_slots[i] = Some(entry);
                    loaded_slots += 1;
                    needs_rewrite = true;
                    info!("slot_{}.mpk in legacy format — will rewrite", i + 1);
                }
                DecodeResult::Corrupt(reason) => {
                    quarantine(&path, &reason);
                }
            },
            Err(e) => error!("Cannot read slot_{}.mpk: {}", i + 1, e),
        }
    }

    let ring_path = dir.join("dynamic_ring.mpk");
    let mut loaded_dynamic = 0usize;
    if ring_path.exists() {
        match fs::read(&ring_path) {
            Ok(bytes) => match decode_envelope::<Vec<ClipEntry>>(&bytes, None) {
                DecodeResult::Validated(mut entries) => {
                    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    entries.truncate(ram.capacity);
                    loaded_dynamic = entries.len();
                    ram.dynamic_ring.extend(entries);
                }
                DecodeResult::LegacyUnvalidated(mut entries) => {
                    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    loaded_dynamic = entries.len();
                    ram.dynamic_ring.extend(entries);
                    needs_rewrite = true;
                    info!("dynamic_ring.mpk in legacy format — will rewrite");
                }
                DecodeResult::Corrupt(reason) => {
                    quarantine(&ring_path, &reason);
                }
            },
            Err(e) => error!("Cannot read dynamic_ring.mpk: {}", e),
        }
    }

    info!(
        "Loaded from disk — {} static slots, {} dynamic entries",
        loaded_slots, loaded_dynamic
    );

    // Rewrite once now so subsequent loads use the new format and get full
    // integrity checking. Force the flag so flush_force runs even though
    // nothing user-visible changed.
    if needs_rewrite {
        ram.dirty = true; // public field per current ram.rs
        if let Err(e) = flush_force(ram) {
            warn!("Legacy → envelope rewrite failed (non-fatal): {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_roundtrip() {
        let payload_data = vec!["entry1".to_string(), "entry2".to_string()];
        let msgpack_payload = rmp_serde::to_vec(&payload_data).unwrap();
        
        let enveloped = wrap_envelope(&msgpack_payload, 2);
        
        let result = decode_envelope::<Vec<String>>(&enveloped, Some(2));
        match result {
            DecodeResult::Validated(decoded) => assert_eq!(decoded, payload_data),
            _ => panic!("Expected Validated result"),
        }
    }

    #[test]
    fn test_legacy_format_fallback() {
        let payload_data = vec!["legacy_entry".to_string()];
        let msgpack_payload = rmp_serde::to_vec(&payload_data).unwrap();
        
        let result = decode_envelope::<Vec<String>>(&msgpack_payload, Some(1));
        match result {
            DecodeResult::LegacyUnvalidated(decoded) => assert_eq!(decoded, payload_data),
            _ => panic!("Expected LegacyUnvalidated result"),
        }
    }

    #[test]
    fn test_crc_corruption_detection() {
        let payload_data = vec!["test_data".to_string()];
        let msgpack_payload = rmp_serde::to_vec(&payload_data).unwrap();
        let mut enveloped = wrap_envelope(&msgpack_payload, 1);
        
        // Corrupt the payload section
        enveloped[12] ^= 0xFF; 
        
        let result = decode_envelope::<Vec<String>>(&enveloped, Some(1));
        match result {
            DecodeResult::Corrupt(reason) => assert!(reason.contains("checksum mismatch")),
            _ => panic!("Expected Corrupt result due to CRC mismatch"),
        }
    }

    #[test]
    fn test_truncation_detection() {
        let payload_data = vec!["test_data".to_string()];
        let msgpack_payload = rmp_serde::to_vec(&payload_data).unwrap();
        let mut enveloped = wrap_envelope(&msgpack_payload, 1);
        
        // Truncate to simulate partial write
        enveloped.truncate(10); 
        
        let result = decode_envelope::<Vec<String>>(&enveloped, Some(1));
        match result {
            DecodeResult::Corrupt(reason) => assert!(reason.contains("envelope truncated")),
            _ => panic!("Expected Corrupt result due to truncation"),
        }
    }

    #[test]
    fn test_count_mismatch_detection() {
        let payload_data = vec!["test_data".to_string()];
        let msgpack_payload = rmp_serde::to_vec(&payload_data).unwrap();
        
        let enveloped = wrap_envelope(&msgpack_payload, 1);
        let result = decode_envelope::<Vec<String>>(&enveloped, Some(5));
        
        match result {
            DecodeResult::Corrupt(reason) => assert!(reason.contains("count 1 != caller expected 5")),
            _ => panic!("Expected Corrupt result due to count mismatch"),
        }
    }
}