#!/usr/bin/env bash
set -euo pipefail

# Builds ClipWallet and packages it into a distributable zip.

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//' | sed 's/"//')
ARCH=$(uname -m)

case "$ARCH" in
    arm64)  TARGET="aarch64-apple-darwin" ;;
    x86_64) TARGET="x86_64-apple-darwin"  ;;
    *)      echo "Unsupported arch"; exit 1 ;;
esac

DIST_DIR="dist"
ZIP_NAME="clipwallet-${VERSION}-${TARGET}.zip"

echo "Building ClipWallet v${VERSION} for ${TARGET}..."

# Build
cargo build --release --target "$TARGET" 2>/dev/null \
    || cargo build --release   # fallback to default target

BINARY="target/${TARGET}/release/clipwallet"
[ -f "$BINARY" ] || BINARY="target/release/clipwallet"

# Sign (skip in CI by setting SKIP_CODESIGN=1)
if [ "${SKIP_CODESIGN:-0}" != "1" ]; then
  if command -v codesign >/dev/null 2>&1; then
    codesign --sign - --force "$BINARY"
    echo "Binary signed ✓"
  else
    echo "codesign not available — skipping signing"
  fi
else
  echo "SKIP_CODESIGN=1 — skipping codesign"
fi

# Package
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/clipwallet"

cp "$BINARY"    "$DIST_DIR/clipwallet/clipwallet"
cp "install.sh" "$DIST_DIR/clipwallet/install.sh"
chmod +x "$DIST_DIR/clipwallet/install.sh"

# Also produce a standalone per-architecture binary asset for Releases
# Named: clipwallet-<target> (e.g. clipwallet-aarch64-apple-darwin)
ARCHIVE_BINARY_NAME="clipwallet-${TARGET}"
cp "$BINARY" "$DIST_DIR/${ARCHIVE_BINARY_NAME}"
chmod +x "$DIST_DIR/${ARCHIVE_BINARY_NAME}"
echo "Per-arch binary ready: ${DIST_DIR}/${ARCHIVE_BINARY_NAME}"

cat > "$DIST_DIR/clipwallet/README.txt" << EOF
ClipWallet v${VERSION} — Persistent Clipboard Manager for macOS
================================================================

INSTALL (double-click method):
  1. Open Terminal
  2. cd into this folder
  3. Run: ./install.sh

INSTALL (one-line):
  curl -fsSL https://github.com/shaaravraghu/ClipWallet/releases/latest/download/install.sh | sh

AFTER INSTALL:
  Grant Accessibility access:
  System Settings → Privacy & Security → Accessibility → + → clipwallet

HOTKEYS:
  Cmd+Opt+C+[1-9]       Copy into static slot
  Cmd+Opt+V+[1-9]       Paste from static slot
  Cmd+Opt+X+[1-9]       Cut into static slot
  Cmd+Opt+Tab           Navigate forward (dynamic ring)
  Cmd+Opt+Shift+Tab     Navigate backward
  Cmd+Opt+Tab+Esc       Delete current entry
  Cmd+Opt+C             Dynamic copy (no digit)
  Cmd+Opt+X             Dynamic cut (no digit)

COMMANDS:
  clipwallet status
  clipwallet vault-list
  clipwallet vault-delete <id>
  clipwallet vault-rotate
  clipwallet uninstall
  clipwallet uninstall --purge

LOGS:
  tail -f ~/.clipwallet/logs/out.log
EOF

# Zip
cd "$DIST_DIR"
zip -r "$ZIP_NAME" clipwallet/
cd ..

echo ""
echo "Package ready: ${DIST_DIR}/${ZIP_NAME}"
echo "Contents:"
unzip -l "${DIST_DIR}/${ZIP_NAME}"
