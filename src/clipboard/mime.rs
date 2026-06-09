//! Detects the most specific data type from raw bytes.
//! Used to classify clipboard content before storing.

use crate::clipboard::types::ClipData;

#[derive(Debug, Clone, PartialEq)]
pub enum DetectedType {
    PlainText,
    RichText, // RTF
    ImagePng,
    ImageJpeg,
    ImageOther,
    FilePaths,
    Binary,
}

/// Detect type from raw bytes using magic bytes + infer crate.
pub fn detect_bytes(bytes: &[u8]) -> DetectedType {
    // Try infer crate first (magic byte detection)
    if let Some(kind) = infer::get(bytes) {
        match kind.mime_type() {
            "image/png" => return DetectedType::ImagePng,
            "image/jpeg" => return DetectedType::ImageJpeg,
            m if m.starts_with("image/") => return DetectedType::ImageOther,
            _ => {}
        }
    }

    // RTF magic: starts with "{\rtf"
    if bytes.starts_with(b"{\\rtf") {
        return DetectedType::RichText;
    }

    // Valid UTF-8 = plain text
    if std::str::from_utf8(bytes).is_ok() {
        return DetectedType::PlainText;
    }

    DetectedType::Binary
}

/// Convert raw image bytes to normalised PNG.
/// Ensures all images are stored as PNG regardless of source format.
pub fn normalise_image(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use image::io::Reader as ImageReader;
    use std::io::Cursor;

    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageOutputFormat::Png)?;
    Ok(out)
}
