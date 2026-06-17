# Changelog

## [Unreleased]
### Added
- Large clipboard entries (images, binary, and rich-text blobs) at or above a
  configurable size threshold are now spilled to disk and held in RAM only as a
  lightweight pointer, loaded lazily at paste time and released immediately
  afterwards (#28). New `spill_threshold_bytes` config option (default 1 MiB,
  `0` to disable). Orphaned blob files are reclaimed by reference-counted GC on
  flush and at startup. Status output now reports on-disk blob count and size.

## [0.1.0] - 2026-05-15
### Added
- Initial open-source release for macOS.
- Static and Dynamic clipboard modes.
- AES-256-GCM encrypted vault.
- Persistence via RAM and Disk storage.
