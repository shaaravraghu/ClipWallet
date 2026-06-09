use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type EntryId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClipData {
    PlainText(String),
    RichText(Vec<u8>),
    /// PNG bytes + original dimensions for arboard reconstruction
    Image {
        bytes: Vec<u8>,
        width: usize,
        height: usize,
    },
    /// Pointer-style: path only, file bytes never loaded
    FilePath(Vec<PathBuf>),
    Binary(Vec<u8>),
}

impl ClipData {
    pub fn type_label(&self) -> &'static str {
        match self {
            ClipData::PlainText(_) => "PlainText",
            ClipData::RichText(_) => "RichText",
            ClipData::Image { .. } => "Image",
            ClipData::FilePath(_) => "FilePath",
            ClipData::Binary(_) => "Binary",
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            ClipData::PlainText(s) => s.len(),
            ClipData::RichText(b) => b.len(),
            ClipData::Image { bytes, .. } => bytes.len(),
            ClipData::Binary(b) => b.len(),
            ClipData::FilePath(paths) => paths.iter().map(|p| p.to_string_lossy().len()).sum(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id: EntryId,
    pub timestamp: DateTime<Utc>,
    pub data: ClipData,
    pub encrypted: bool,
    pub label: Option<String>,
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
