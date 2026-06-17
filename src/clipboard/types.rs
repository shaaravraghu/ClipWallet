use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type EntryId = u64;

/// Identifies which concrete byte-backed variant a spilled blob was produced
/// from, carrying the small amount of metadata needed to (a) render previews
/// and report sizes without touching disk, and (b) faithfully reconstruct the
/// original [`ClipData`] when the blob is loaded back at paste time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpilledKind {
    RichText,
    Image { width: usize, height: usize },
    Binary,
}

impl SpilledKind {
    pub fn type_label(&self) -> &'static str {
        match self {
            SpilledKind::RichText     => "RichText",
            SpilledKind::Image { .. } => "Image",
            SpilledKind::Binary       => "Binary",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipData {
    PlainText(String),
    RichText(Vec<u8>),
    /// PNG bytes + original dimensions for arboard reconstruction
    Image { bytes: Vec<u8>, width: usize, height: usize },
    /// Pointer-style: path only, file bytes never loaded
    FilePath(Vec<PathBuf>),
    Binary(Vec<u8>),
    /// A large entry whose bytes have been spilled to disk
    /// (`~/.clipwallet/blobs/<blob_id>.blob`). Only this lightweight descriptor
    /// is held in RAM; the bytes are loaded lazily at paste time and released
    /// immediately afterwards. See `storage::spill` and issue #28.
    ///
    /// MUST remain the LAST variant. MessagePack (rmp-serde) encodes enum
    /// variants by their declaration index, so appending keeps every existing
    /// on-disk store readable after an upgrade.
    Spilled {
        /// Id of the backing blob file. Equal to the owning entry's id.
        blob_id: EntryId,
        /// Original variant + the metadata required to rebuild it on hydration.
        kind:    SpilledKind,
        /// Logical size of the spilled bytes — what the entry would occupy in
        /// RAM if resident. Reported by [`ClipData::size_bytes`] so logs and
        /// status output stay meaningful even while the bytes are on disk.
        size:    usize,
        /// CRC32 of the spilled bytes. Lets two spilled descriptors be compared
        /// for content equality (consecutive-duplicate dedup) without reading
        /// either blob. `blob_id` is intentionally excluded from equality.
        hash:    u32,
    },
}

impl ClipData {
    pub fn type_label(&self) -> &'static str {
        match self {
            ClipData::PlainText(_)          => "PlainText",
            ClipData::RichText(_)           => "RichText",
            ClipData::Image { .. }          => "Image",
            ClipData::FilePath(_)           => "FilePath",
            ClipData::Binary(_)             => "Binary",
            ClipData::Spilled { kind, .. }  => kind.type_label(),
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            ClipData::PlainText(s)          => s.len(),
            ClipData::RichText(b)           => b.len(),
            ClipData::Image { bytes, .. }   => bytes.len(),
            ClipData::Binary(b)             => b.len(),
            ClipData::FilePath(paths)       => {
                paths.iter().map(|p| p.to_string_lossy().len()).sum()
            }
            ClipData::Spilled { size, .. }  => *size,
        }
    }

    /// True when the bytes for this entry currently live on disk rather than in
    /// the active ring or slot (i.e. it is a [`ClipData::Spilled`] descriptor).
    pub fn is_spilled(&self) -> bool {
        matches!(self, ClipData::Spilled { .. })
    }
}

// Manual `PartialEq` (rather than `#[derive]`) so that two `Spilled` descriptors
// are considered equal when they describe identical *content*, regardless of
// which blob file backs them. This preserves the dynamic ring's
// consecutive-duplicate dedup for large entries without loading any bytes. All
// other variants compare by value exactly as a derived impl would.
impl PartialEq for ClipData {
    fn eq(&self, other: &Self) -> bool {
        use ClipData::*;
        match (self, other) {
            (PlainText(a), PlainText(b)) => a == b,
            (RichText(a), RichText(b))   => a == b,
            (
                Image { bytes: ab, width: aw, height: ah },
                Image { bytes: bb, width: bw, height: bh },
            ) => ab == bb && aw == bw && ah == bh,
            (FilePath(a), FilePath(b)) => a == b,
            (Binary(a), Binary(b))     => a == b,
            (
                Spilled { kind: ak, size: asz, hash: ah, .. },
                Spilled { kind: bk, size: bsz, hash: bh, .. },
            ) => ak == bk && asz == bsz && ah == bh,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id:        EntryId,
    pub timestamp: DateTime<Utc>,
    pub data:      ClipData,
    pub encrypted: bool,
    pub label:     Option<String>,
}

impl ClipEntry {
    pub fn new(id: EntryId, data: ClipData) -> Self {
        Self {
            id,
            timestamp: Utc::now(),
            data,
            encrypted: false,
            label: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spilled_equality_ignores_blob_id() {
        // Same content spilled under two different ids must compare equal so the
        // ring still dedups consecutive duplicate large copies.
        let a = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::Binary,
            size: 1024,
            hash: 0xDEAD_BEEF,
        };
        let b = ClipData::Spilled {
            blob_id: 2, // different backing blob
            kind: SpilledKind::Binary,
            size: 1024,
            hash: 0xDEAD_BEEF,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn spilled_equality_distinguishes_content() {
        let base = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::Binary,
            size: 1024,
            hash: 0x1111_1111,
        };
        let diff_hash = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::Binary,
            size: 1024,
            hash: 0x2222_2222,
        };
        let diff_size = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::Binary,
            size: 2048,
            hash: 0x1111_1111,
        };
        let diff_kind = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::RichText,
            size: 1024,
            hash: 0x1111_1111,
        };
        assert_ne!(base, diff_hash);
        assert_ne!(base, diff_size);
        assert_ne!(base, diff_kind);
    }

    #[test]
    fn spilled_never_equals_resident() {
        let spilled = ClipData::Spilled {
            blob_id: 1,
            kind: SpilledKind::Binary,
            size: 3,
            hash: crc32fast::hash(&[1, 2, 3]),
        };
        let resident = ClipData::Binary(vec![1, 2, 3]);
        assert_ne!(spilled, resident);
    }

    #[test]
    fn type_label_and_size_reflect_underlying_kind() {
        let img = ClipData::Spilled {
            blob_id: 7,
            kind: SpilledKind::Image { width: 800, height: 600 },
            size: 50_000,
            hash: 0,
        };
        assert_eq!(img.type_label(), "Image");
        assert_eq!(img.size_bytes(), 50_000);
        assert!(img.is_spilled());

        let txt = ClipData::PlainText("hello".into());
        assert_eq!(txt.type_label(), "PlainText");
        assert_eq!(txt.size_bytes(), 5);
        assert!(!txt.is_spilled());
    }

    #[test]
    fn resident_equality_unchanged() {
        assert_eq!(
            ClipData::PlainText("x".into()),
            ClipData::PlainText("x".into())
        );
        assert_ne!(
            ClipData::PlainText("x".into()),
            ClipData::PlainText("y".into())
        );
        assert_eq!(ClipData::Binary(vec![1, 2]), ClipData::Binary(vec![1, 2]));
        assert_ne!(ClipData::Binary(vec![1, 2]), ClipData::Binary(vec![1, 3]));
    }
}
