//! Disk-backed spill store for large clipboard entries (issue #28).
//!
//! Large clipboard payloads — images and binary blobs — used to be held as
//! fully-resident `Vec<u8>` byte arrays inside the active ring or static slots.
//! Copying a 50 MB image cost 50 MB of RAM the moment it was captured and kept
//! costing it whether or not the entry was ever pasted.
//!
//! This module makes those payloads behave like *pointers*. When an entry's
//! payload meets or exceeds a configurable threshold, [`maybe_spill`] writes the
//! bytes to a blob file on disk and replaces the in-RAM payload with a
//! lightweight [`ClipData::Spilled`] descriptor (a few dozen bytes). The bytes
//! are read back with [`hydrate`] only at paste time and dropped again the
//! instant the system pasteboard has copied them.
//!
//! ## Blob lifecycle
//!
//! Rather than thread disk deletes through every `RamStore` mutation (evict,
//! delete, overwrite, encrypt-to-vault, …), blob lifetime is managed by
//! reference-based garbage collection. [`gc`] removes any blob file whose id is
//! not referenced by a live entry. It runs on every flush (a mutation always
//! marks the store dirty, so an orphaned blob is reclaimed within one flush
//! interval), at shutdown, and at startup — the last of which also reclaims
//! blobs orphaned by a hard crash (`SIGKILL`, power-off) that bypassed the
//! graceful path.

use crate::clipboard::types::{ClipData, ClipEntry, EntryId, SpilledKind};
use crate::storage::ram::RamStore;
use dirs::home_dir;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Extension used for finalised blob files. Tmp files use `.tmp`.
const BLOB_EXT: &str = "blob";

/// `~/.clipwallet/blobs` — sibling of `store/` and `vault/`.
pub fn blob_dir() -> PathBuf {
    home_dir()
        .expect("No home dir")
        .join(".clipwallet")
        .join("blobs")
}

fn blob_path(id: EntryId) -> PathBuf {
    blob_dir().join(format!("{}.{}", id, BLOB_EXT))
}

// ─── Durable write ──────────────────────────────────────────────────────────
// Same write → fsync → rename → fsync-parent discipline as storage::disk, so a
// crash mid-spill can never leave a half-written blob that a later hydrate would
// silently read as truncated content.

fn atomic_write(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?; // bytes reach stable storage
    }
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all(); // rename entry reaches stable storage
        }
    }
    Ok(())
}

// ─── Blob primitives ──────────────────────────────────────────────────────────

pub fn write_blob(id: EntryId, bytes: &[u8]) -> std::io::Result<()> {
    let dir = blob_dir();
    fs::create_dir_all(&dir)?;
    atomic_write(&blob_path(id), bytes)
}

pub fn read_blob(id: EntryId) -> std::io::Result<Vec<u8>> {
    fs::read(blob_path(id))
}

pub fn delete_blob(id: EntryId) {
    let path = blob_path(id);
    if path.exists() {
        if let Err(e) = fs::remove_file(&path) {
            warn!("Failed to delete blob {}: {}", id, e);
        }
    }
}

/// Remove any orphaned `.tmp` blob files left by an interrupted spill.
pub fn cleanup_tmp_files() {
    let dir = blob_dir();
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tmp") {
                let _ = fs::remove_file(&path);
                warn!("Cleaned up orphaned blob tmp file: {:?}", path);
            }
        }
    }
}

// ─── Spill / hydrate ──────────────────────────────────────────────────────────

/// If `entry` carries a spillable byte payload (RichText, Image, or Binary) of
/// at least `threshold` bytes, write those bytes to a blob keyed by the entry id
/// and return the entry with its payload replaced by a [`ClipData::Spilled`]
/// descriptor. Otherwise the entry is returned unchanged.
///
/// A `threshold` of `0` disables spilling entirely. `PlainText` (kept resident
/// for previews) and `FilePath` (already a pointer) are never spilled, nor is an
/// already-`Spilled` descriptor.
///
/// On any disk error the entry is returned **unspilled** with its bytes intact —
/// a transient memory cost is always preferable to losing the user's clipboard.
pub fn maybe_spill(entry: ClipEntry, threshold: usize) -> ClipEntry {
    if threshold == 0 || entry.data.is_spilled() || entry.data.size_bytes() < threshold {
        return entry;
    }

    let ClipEntry { id, timestamp, data, encrypted, label } = entry;

    let (bytes, kind) = match data {
        ClipData::RichText(b) => (b, SpilledKind::RichText),
        ClipData::Binary(b)   => (b, SpilledKind::Binary),
        ClipData::Image { bytes, width, height } => {
            (bytes, SpilledKind::Image { width, height })
        }
        // Not a spillable byte payload (PlainText / FilePath / already Spilled).
        other => return ClipEntry { id, timestamp, data: other, encrypted, label },
    };

    let size = bytes.len();
    let hash = crc32fast::hash(&bytes);

    let data = match write_blob(id, &bytes) {
        Ok(()) => {
            debug!(
                "Spilled entry id={} ({}, {} bytes) → {:?}",
                id,
                kind.type_label(),
                size,
                blob_path(id)
            );
            // The resident copy is released here — only the descriptor survives.
            drop(bytes);
            ClipData::Spilled { blob_id: id, kind, size, hash }
        }
        Err(e) => {
            warn!(
                "Spill failed for id={} ({}); keeping resident in RAM: {}",
                id,
                kind.type_label(),
                e
            );
            match kind {
                SpilledKind::RichText => ClipData::RichText(bytes),
                SpilledKind::Binary   => ClipData::Binary(bytes),
                SpilledKind::Image { width, height } => {
                    ClipData::Image { bytes, width, height }
                }
            }
        }
    };

    ClipEntry { id, timestamp, data, encrypted, label }
}

/// Return `data` with its bytes resident in memory.
///
/// For a [`ClipData::Spilled`] descriptor this reads the backing blob from disk
/// and reconstructs the original variant, returning [`Cow::Owned`]. For every
/// other variant it borrows the data unchanged ([`Cow::Borrowed`], no copy).
///
/// The owned result holds the only in-RAM copy of a spilled payload and is meant
/// to be short-lived: sync it to the system pasteboard, then let it drop.
pub fn hydrate(data: &ClipData) -> anyhow::Result<std::borrow::Cow<'_, ClipData>> {
    use std::borrow::Cow;
    match data {
        ClipData::Spilled { blob_id, kind, size, .. } => {
            let bytes = read_blob(*blob_id)
                .map_err(|e| anyhow::anyhow!("blob {} unreadable: {}", blob_id, e))?;
            if bytes.len() != *size {
                warn!(
                    "Blob {} size {} != descriptor size {} (continuing)",
                    blob_id,
                    bytes.len(),
                    size
                );
            }
            let restored = match kind {
                SpilledKind::RichText => ClipData::RichText(bytes),
                SpilledKind::Binary   => ClipData::Binary(bytes),
                SpilledKind::Image { width, height } => ClipData::Image {
                    bytes,
                    width: *width,
                    height: *height,
                },
            };
            Ok(Cow::Owned(restored))
        }
        other => Ok(Cow::Borrowed(other)),
    }
}

// ─── Garbage collection ─────────────────────────────────────────────────────────

/// Collect the ids of every blob currently referenced by a live entry — across
/// both the static slots and the dynamic ring.
pub fn live_blob_ids(ram: &RamStore) -> HashSet<EntryId> {
    let mut live = HashSet::new();
    for slot in ram.static_slots.iter().flatten() {
        if let ClipData::Spilled { blob_id, .. } = slot.data {
            live.insert(blob_id);
        }
    }
    for entry in ram.dynamic_ring.iter() {
        if let ClipData::Spilled { blob_id, .. } = entry.data {
            live.insert(blob_id);
        }
    }
    live
}

/// Delete every finalised blob file whose id is not present in `live`.
/// Returns the number of blobs removed. `.tmp` files are left for
/// [`cleanup_tmp_files`].
pub fn gc(live: &HashSet<EntryId>) -> usize {
    let dir = blob_dir();
    if !dir.exists() {
        return 0;
    }
    let mut removed = 0usize;
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some(BLOB_EXT) {
                continue;
            }
            let id = match path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<EntryId>().ok())
            {
                Some(id) => id,
                None => continue,
            };
            if !live.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        info!("Blob GC reclaimed {} orphaned blob(s)", removed);
    }
    removed
}

/// Convenience wrapper: garbage-collect against the live set derived from `ram`.
pub fn gc_orphans(ram: &RamStore) -> usize {
    gc(&live_blob_ids(ram))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::types::ClipEntry;
    use std::sync::Mutex;

    // home_dir() reads $HOME, which is process-global. Serialise the disk-backed
    // tests and point $HOME at a unique temp dir so they can't see each other's
    // blobs or a real user's ~/.clipwallet.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        dir: PathBuf,
        prev: Option<String>,
    }

    impl TempHome {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("cw_spill_{}_{}", tag, nanos));
            fs::create_dir_all(&dir).unwrap();
            let prev = std::env::var("HOME").ok();
            std::env::set_var("HOME", &dir);
            TempHome { dir, prev }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn entry(id: EntryId, data: ClipData) -> ClipEntry {
        ClipEntry::new(id, data)
    }

    #[test]
    fn blob_write_read_delete_roundtrip() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("rt");

        let payload = vec![7u8; 4096];
        write_blob(42, &payload).unwrap();
        assert!(blob_path(42).exists());
        assert_eq!(read_blob(42).unwrap(), payload);

        delete_blob(42);
        assert!(!blob_path(42).exists());
    }

    #[test]
    fn maybe_spill_spills_above_threshold_and_hydrates_back() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("spill");

        let original = vec![3u8; 2048];
        let e = entry(100, ClipData::Binary(original.clone()));
        let spilled = maybe_spill(e, 1024);

        // Resident payload is now a descriptor, not the bytes.
        match &spilled.data {
            ClipData::Spilled { blob_id, kind, size, hash } => {
                assert_eq!(*blob_id, 100);
                assert_eq!(*kind, SpilledKind::Binary);
                assert_eq!(*size, 2048);
                assert_eq!(*hash, crc32fast::hash(&original));
            }
            other => panic!("expected Spilled, got {:?}", other),
        }
        assert!(blob_path(100).exists());

        // Hydrate reconstructs the exact original variant + bytes.
        let hydrated = hydrate(&spilled.data).unwrap();
        assert_eq!(*hydrated, ClipData::Binary(original));
    }

    #[test]
    fn maybe_spill_preserves_image_dimensions() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("img");

        let bytes = vec![9u8; 5000];
        let e = entry(7, ClipData::Image { bytes: bytes.clone(), width: 1920, height: 1080 });
        let spilled = maybe_spill(e, 1024);
        let hydrated = hydrate(&spilled.data).unwrap();
        assert_eq!(
            *hydrated,
            ClipData::Image { bytes, width: 1920, height: 1080 }
        );
    }

    #[test]
    fn maybe_spill_leaves_small_and_nonspillable_entries_resident() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("small");

        // Below threshold → unchanged.
        let small = entry(1, ClipData::Binary(vec![0u8; 100]));
        assert!(!maybe_spill(small, 1024).data.is_spilled());

        // PlainText is never spilled even when large.
        let text = entry(2, ClipData::PlainText("x".repeat(10_000)));
        assert!(!maybe_spill(text, 1024).data.is_spilled());

        // FilePath is already a pointer.
        let paths = entry(3, ClipData::FilePath(vec![PathBuf::from("/tmp/a")]));
        assert!(!maybe_spill(paths, 1).data.is_spilled());

        // threshold 0 disables spilling.
        let big = entry(4, ClipData::Binary(vec![0u8; 100_000]));
        assert!(!maybe_spill(big, 0).data.is_spilled());
    }

    #[test]
    fn hydrate_passes_through_resident_data_without_copy() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("passthrough");

        let data = ClipData::PlainText("hello".into());
        let out = hydrate(&data).unwrap();
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert_eq!(*out, data);
    }

    #[test]
    fn hydrate_errors_when_blob_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("missing");

        let descriptor = ClipData::Spilled {
            blob_id: 9999,
            kind: SpilledKind::Binary,
            size: 10,
            hash: 0,
        };
        assert!(hydrate(&descriptor).is_err());
    }

    #[test]
    fn gc_reclaims_only_unreferenced_blobs() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("gc");

        write_blob(1, &[1u8; 64]).unwrap();
        write_blob(2, &[2u8; 64]).unwrap();
        write_blob(3, &[3u8; 64]).unwrap();

        // Live ring references only blob 2 (spilled); slot references blob 3.
        let mut ram = RamStore::new(10);
        ram.dynamic_ring.push_front(entry(
            2,
            ClipData::Spilled { blob_id: 2, kind: SpilledKind::Binary, size: 64, hash: 0 },
        ));
        ram.static_slots[0] = Some(entry(
            3,
            ClipData::Spilled { blob_id: 3, kind: SpilledKind::Binary, size: 64, hash: 0 },
        ));

        let removed = gc_orphans(&ram);
        assert_eq!(removed, 1); // blob 1 only
        assert!(!blob_path(1).exists());
        assert!(blob_path(2).exists());
        assert!(blob_path(3).exists());
    }

    #[test]
    fn cleanup_removes_tmp_blobs() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _home = TempHome::new("tmp");

        fs::create_dir_all(blob_dir()).unwrap();
        let tmp = blob_dir().join("123.tmp");
        fs::write(&tmp, b"partial").unwrap();
        assert!(tmp.exists());

        cleanup_tmp_files();
        assert!(!tmp.exists());
    }
}
