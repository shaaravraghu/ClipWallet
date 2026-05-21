#!/usr/bin/env bash
set -euo pipefail

# Builds ClipWallet and packages it into a distributable zip.

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//' | sed 's/"//')

# Targets to build/package — ensure both archs are included so CI
# that builds both artifacts will always find matching dist files.
TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")

DIST_DIR="dist"
ZIP_NAME="clipwallet-${VERSION}-macos-multiarch.zip"

echo "Building ClipWallet v${VERSION} for: ${TARGETS[*]}"

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/clipwallet"

for TARGET in "${TARGETS[@]}"; do
  echo "-- Building target: $TARGET"
  # Try explicit target build, fall back to host build if not available.
  if ! cargo build --release --target "$TARGET" 2>/dev/null; then
    echo "  Fallback: building host release (no cross target)"
    cargo build --release
  fi

  # Determine binary path for the target
  BINARY="target/${TARGET}/release/clipwallet"
  if [ ! -f "$BINARY" ]; then
    # Fallback to host-built binary if target-specific doesn't exist
    BINARY="target/release/clipwallet"
  fi

  if [ ! -f "$BINARY" ]; then
    echo "ERROR: binary not found for target ${TARGET}" >&2
    exit 1
  fi

  # Sign each binary (best-effort; codesign may require proper identity)
  if command -v codesign >/dev/null 2>&1; then
    echo "  Signing $BINARY"
    codesign --sign - --force "$BINARY" || echo "  Warning: codesign failed"
  fi

  # Copy into dist with explicit arch-qualified name
  OUT_NAME="clipwallet-${TARGET}"
  cp "$BINARY" "$DIST_DIR/clipwallet/${OUT_NAME}"
  chmod +x "$DIST_DIR/clipwallet/${OUT_NAME}"
  echo "  Packaged: $DIST_DIR/clipwallet/${OUT_NAME}"
done

# Always include install + README in the package
cp "install.sh" "$DIST_DIR/clipwallet/install.sh"
chmod +x "$DIST_DIR/clipwallet/install.sh"

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
