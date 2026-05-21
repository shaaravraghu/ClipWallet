<p align="center">
  <h1 align="center">ClipWallet</h1>
  <p align="center">
    <strong>A persistent, encrypted, multi-slot clipboard manager for macOS.</strong>
  </p>
  <p align="center">
    <a href="#quick-start">Quick Start</a> •
    <a href="#installation">Installation</a> •
    <a href="#hotkeys">Hotkeys</a> •
    <a href="#modes">Modes</a> •
    <a href="#usage-examples">Examples</a> •
    <a href="#commands">Commands</a> •
    <a href="#faq">FAQ</a> •
    <a href="#troubleshooting">Troubleshooting</a> •
    <a href="#architecture">Architecture</a> •
    <a href="#contributing">Contributing</a>
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/platform-macOS-blue?style=flat-square&logo=apple" alt="macOS">
    <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License">
    <img src="https://img.shields.io/badge/version-0.1.0-purple?style=flat-square" alt="Version">
    <img src="https://img.shields.io/badge/GSSoC'26-Contributor%20Friendly-yellow?style=flat-square" alt="GSSoC">
  </p>
</p>

---

## The Problem

You use `Ctrl+C` / `Cmd+C` a million times a day. Every single time, your previous copy is **gone forever**. You're one paste away from losing that URL, that code snippet, that address you copied 30 seconds ago.

## The Solution

**ClipWallet** turns your clipboard into a **wallet** — multiple addressable memory slots that persist across reboots, survive crashes, and encrypt sensitive data automatically. No GUI, no menubar clutter. Pure keyboard-native power.

## Product Demo
https://www.youtube.com/watch?v=WVQdXm9cpf4

## Community
https://discord.gg/X8Hr8P9J

---

## Features

| Feature | Description |
|---------|-------------|
| 🗂️ **Multi-slot clipboard** | 9 static slots or a 50-entry dynamic ring buffer |
| 💾 **Persistent storage** | Clipboard history survives reboots and crashes |
| 🔐 **AES-256-GCM encryption** | Vault-encrypted entries with macOS Keychain key storage |
| ⌨️ **Keyboard-native** | No GUI — controlled entirely through hotkey chords |
| 🖼️ **Rich data types** | Supports text, RTF, images (PNG/JPEG), file paths, binary data |
| 🔄 **Two operating modes** | Static (addressed slots) or Dynamic (recency ring) |
| 🚀 **Background daemon** | Runs as a `launchd` login agent, auto-starts on boot |
| 📦 **Single binary** | Zero-dependency Rust binary — no Electron, no frameworks |
| 🛡️ **Crash-safe writes** | Atomic tmp+rename disk persistence |
| 📁 **Pointer-style files** | File paths stored, never file contents — no RAM bloat |

---

## Quick Start

Get ClipWallet running in under 2 minutes:

```bash
# Download and install
curl -fsSL https://github.com/shaaravraghu/ClipWallet/releases/latest/download/install.sh | sh

# Choose your mode (Static or Dynamic)
clipwallet change mode

# Grant Accessibility permission (required!)
# System Settings → Privacy & Security → Accessibility → + → clipwallet

# Verify it's running
clipwallet status
```

**Test it:** Select some text, press `Cmd+Opt+C+1`, then paste anywhere with `Cmd+Opt+V+1` ✓

---
## Installation

Having trouble or prefer to build it yourself? Check out our [Beginner-Friendly Manual Installation Guide](MANUAL_INSTRUCTIONS.md) for step-by-step instructions for Windows, macOS, and Linux.

---

## Usage Examples

### Real-World Workflows

<details>
<summary><b>👨‍💻 Developer: API Testing</b></summary>

**Scenario:** Testing an API endpoint with multiple parameters
**Scenario:** Testing an API endpoint with multiple parameters

```bash
# Copy different values to different slots
1. Copy API endpoint URL → Cmd+Opt+C+1
2. Copy authentication token → Cmd+Opt+C+2
3. Copy JSON payload → Cmd+Opt+C+3

# Switch to terminal and paste in sequence
Cmd+Opt+V+1  # Paste endpoint
Cmd+Opt+V+2  # Paste token
Cmd+Opt+V+3  # Paste payload
```

**Before ClipWallet:** Copy URL → Switch app → Paste → Go back → Copy token → **URL is gone!** 😢

**After ClipWallet:** Copy all three → Paste anywhere, anytime ✓
</details>

<details>
<summary><b>✍️ Content Writer: Research Notes</b></summary>

**Scenario:** Writing an article with multiple source quotes

```bash
# Save quotes from different sources
Quote 1 from Source A → Cmd+Opt+C+1
Quote 2 from Source B → Cmd+Opt+C+2
Quote 3 from Source C → Cmd+Opt+C+3
Quote 4 from Source D → Cmd+Opt+C+4
Quote 5 from Source E → Cmd+Opt+C+5

# Write your article without losing any quotes
# Paste quotes as needed:
Cmd+Opt+V+1, Cmd+Opt+V+2, etc.
```

**Benefit:** No more switching back and forth between sources and your document!
</details>

<details>
<summary><b>🎨 Designer: Asset Management</b></summary>

**Scenario:** Working with multiple color codes and file paths

```bash
# Store frequently-used values
Primary color (#FF5733) → Cmd+Opt+C+1
Secondary color (#33FF57) → Cmd+Opt+C+2
Logo file path → Cmd+Opt+C+3
Icon directory → Cmd+Opt+C+4
```

**Benefit:** Instant access to commonly-used values without searching!
</details>

<details>
<summary><b>⚙️ System Admin: Server Management</b></summary>

**Scenario:** Managing multiple servers

```bash
# Store server addresses and commands
Production server IP → Cmd+Opt+C+1
Staging server IP → Cmd+Opt+C+2
Common SSH command → Cmd+Opt+C+3
Deployment script → Cmd+Opt+C+4
```

**Benefit:** Quick access to frequently-used commands and addresses!
</details>

---

## Developer Workflow

### For Contributors

<details>
<summary><b>🔍 Watch Live Logs While Testing</b></summary>

```bash
# Watch logs in real-time
tail -f ~/.clipwallet/logs/out.log

# In another terminal, test your hotkeys
# You'll see debug output for every action
```
</details>

<details>
<summary><b>🐛 Run in Foreground with Debug Output</b></summary>

Useful when you want real-time structured logs without running as a daemon:

```bash
RUST_LOG=debug clipwallet run
```

This will show detailed logs including:
- Hotkey detection
- Clipboard operations
- Storage operations
- Encryption/decryption
</details>

<details>
<summary><b>🔄 Stop, Rebuild, and Restart After Code Changes</b></summary>

```bash
# Complete rebuild cycle (run as one command)
clipwallet uninstall && \
cargo build --release && \
sudo cp target/release/clipwallet /usr/local/bin/clipwallet && \
sudo codesign --sign - --force /usr/local/bin/clipwallet && \
clipwallet install
```

> **Tip:** Run this after any source code change to get a clean re-install cycle.
</details>

<details>
<summary><b>⚡ Quick Reinstall (No Rebuild)</b></summary>

If you've only changed config or need to reinstall the daemon:

```bash
clipwallet uninstall && clipwallet install
```
</details>

---

## Hotkeys

### Static Mode — Addressed Slots (1-9)

| Hotkey | Action |
|--------|--------|
| `Cmd + Opt + C + [1-9]` | **Copy** selection into slot N |
| `Cmd + Opt + V + [1-9]` | **Paste** from slot N |
| `Cmd + Opt + X + [1-9]` | **Cut** selection into slot N |
| `Cmd + Opt + Tab` | **Navigate forward** through occupied slots |
| `Cmd + Opt + Shift + Tab` | **Navigate backward** through occupied slots |
| `Cmd + Opt + Tab + Esc` | **Delete** entry at current cursor position |

### Dynamic Mode — Recency Ring

| Hotkey | Action |
|--------|--------|
| `Cmd + Opt + C` | **Copy** selection → pushes to ring front |
| `Cmd + Opt + X` | **Cut** selection → pushes to ring front |
| `Cmd + Opt + V` | **Paste** entry at current cursor position |
| `Cmd + Opt + Tab` | **Navigate forward** (→ older entries) |
| `Cmd + Opt + Shift + Tab` | **Navigate backward** (→ newer entries) |
| `Cmd + Opt + Tab + Esc` | **Delete** entry at current cursor position |

> In both modes, navigating via Tab syncs the selected entry to the system clipboard. Press `Cmd+V` to paste it anywhere.

---

## Modes

### Static Mode

Nine independent, directly addressed slots. Think of them as numbered pockets:

```
Slot 1: [URL]     Slot 4: [empty]   Slot 7: [image]
Slot 2: [code]    Slot 5: [address]  Slot 8: [empty]
Slot 3: [empty]   Slot 6: [email]   Slot 9: [RTF doc]
```

Copy to slot 3 with `Cmd+Opt+C+3`, paste it anytime with `Cmd+Opt+V+3`.

### Dynamic Mode

A recency-ordered ring buffer (up to 50 entries). Every copy pushes to the front. Tab cycles through history:

```
[newest] → [2nd] → [3rd] → ... → [oldest]
    ▲                                  │
    └──────────────────────────────────┘
```

After 10 seconds of idle, the cursor auto-resets to the most recent entry.

### Switching Modes

```bash
clipwallet change mode
```

---

## FAQ

<details>
<summary><b>🔒 Is my clipboard data sent anywhere? (Privacy)</b></summary>

**No.** All clipboard data stays **100% local** on your Mac. ClipWallet:
- Does NOT connect to the internet
- Does NOT send data to any server
- Does NOT share data with third parties
- Stores everything in `~/.clipwallet/` on your local disk

Your data never leaves your machine.
</details>

<details>
<summary><b>🔐 How secure is the encryption?</b></summary>

ClipWallet uses **AES-256-GCM** (authenticated encryption) with:
- 256-bit encryption keys
- Keys stored in macOS Keychain (system-level security)
- Automatic key generation on first run
- Key rotation support via `clipwallet vault-rotate`

This is the same encryption standard used by banks and government agencies.
</details>

<details>
<summary><b>⚡ What's the performance impact?</b></summary>

ClipWallet is extremely lightweight:
- **RAM usage:** ~10-15 MB
- **CPU usage:** Negligible (only active during hotkey presses)
- **Disk usage:** Depends on clipboard history (typically <10 MB)
- **Battery impact:** Minimal

Built in Rust for maximum performance and efficiency.
</details>

<details>
<summary><b>🔑 Does this work with password managers?</b></summary>

**Yes**, but with a caveat:

ClipWallet works alongside password managers like 1Password, Bitwarden, and LastPass. However:

⚠️ **Do NOT store passwords in ClipWallet's persistent slots**

Password managers clear the clipboard after a timeout for security. ClipWallet's persistent storage would bypass this protection.

**Recommendation:** Use Dynamic mode for general clipboard management, and let your password manager handle sensitive credentials.
</details>

<details>
<summary><b>📊 What happens when I reach 50 entries in Dynamic mode?</b></summary>

The oldest entry is **automatically deleted** when you add the 51st entry. This is a rolling buffer:

```
[newest] → [2nd] → [3rd] → ... → [49th] → [50th]
                                            ↓
                                      (deleted when 51st is added)
```

You can adjust this limit by modifying the source code (see `src/storage/ram.rs`).
</details>

<details>
<summary><b>⌨️ Can I customize the hotkeys?</b></summary>

**Not yet.** Custom hotkey configuration is on the roadmap for a future release.

Current hotkeys are:
- Static: `Cmd+Opt+C/V/X+[1-9]`
- Dynamic: `Cmd+Opt+C/V/X` (no digit)
- Navigation: `Cmd+Opt+Tab`

Follow the [Roadmap](#roadmap) for updates on this feature.
</details>

<details>
<summary><b>🔄 Does this conflict with other clipboard managers?</b></summary>

**Possibly.** Running multiple clipboard managers simultaneously can cause conflicts:
- Hotkey collisions
- Race conditions when accessing the clipboard
- Unexpected behavior

**Recommendation:** Use one clipboard manager at a time. If you want to try ClipWallet, disable other clipboard managers first.
</details>

<details>
<summary><b>🚫 Which apps don't work with programmatic paste?</b></summary>

Some apps block programmatic clipboard access for security:
- Password fields in browsers
- Secure input fields (marked with 🔒)
- Some banking/financial apps
- Terminal apps with secure input mode enabled

In these cases, use the native `Cmd+V` after navigating to the entry with `Cmd+Opt+Tab`.
</details>

<details>
<summary><b>🗑️ How do I completely remove ClipWallet?</b></summary>

```bash
# Full uninstall (removes everything)
clipwallet uninstall --purge
```

This will:
- Remove the launchd daemon
- Delete all clipboard history
- Remove encryption keys from Keychain
- Delete the binary from `/usr/local/bin/`
- Remove `~/.clipwallet/` directory

For partial uninstall options, see [Uninstall](#uninstall).
</details>

---

## Troubleshooting

<details>
<summary><b>❌ Hotkeys don't work / Nothing happens when I press hotkeys</b></summary>

**Most common cause:** Accessibility permission not granted

**Solution:**
1. Check if permission is granted:
   ```
   System Settings → Privacy & Security → Accessibility
   ```
2. Ensure `clipwallet` is in the list and **checked**
3. If not in the list, click **+** and add `/usr/local/bin/clipwallet`
4. Restart the daemon:
   ```bash
   clipwallet uninstall && clipwallet install
   ```
5. Test with a simple hotkey: Select text → `Cmd+Opt+C+1`
6. Check logs for errors:
   ```bash
   tail -f ~/.clipwallet/logs/out.log
   ```
</details>

<details>
<summary><b>🚫 "Operation not permitted" error</b></summary>

**Cause:** macOS security restrictions (SIP or Full Disk Access)

**Solution:**
1. Grant Full Disk Access:
   ```
   System Settings → Privacy & Security → Full Disk Access → + → clipwallet
   ```
2. If the issue persists, check System Integrity Protection (SIP) status:
   ```bash
   csrutil status
   ```
3. Ensure the binary is properly code-signed:
   ```bash
   codesign -dv /usr/local/bin/clipwallet
   ```
</details>

<details>
<summary><b>💥 Daemon won't start / Crashes immediately</b></summary>

**Diagnosis:**
```bash
# Check daemon status
clipwallet status

# Check error logs
cat ~/.clipwallet/logs/err.log

# Try running in foreground to see errors
RUST_LOG=debug clipwallet run
```

**Common causes:**
- Port/resource conflict with another process
- Corrupted config file
- Permission issues with `~/.clipwallet/` directory

**Solution:**
```bash
# Clean reinstall
clipwallet uninstall --purge
rm -rf ~/.clipwallet
clipwallet install
```
</details>

<details>
<summary><b>💾 Clipboard doesn't persist after reboot</b></summary>

**Cause:** Daemon not registered with launchd or failed to start

**Solution:**
```bash
# Verify launchd registration
launchctl list | grep clipwallet

# If not listed, reinstall
clipwallet install

# Check if it starts on login
# Restart your Mac and run:
clipwallet status
```
</details>

<details>
<summary><b>🔐 Can't decrypt vault entries / Keychain errors</b></summary>

**Cause:** Encryption key missing or corrupted in Keychain

**Solution:**
```bash
# Rotate the encryption key (re-encrypts all entries)
clipwallet vault-rotate

# If that fails, remove encryption and start fresh
clipwallet remove encryption

# Restart daemon to generate new key
clipwallet uninstall && clipwallet install
```

⚠️ **Warning:** Removing encryption will delete all vault-encrypted entries.
</details>

<details>
<summary><b>🔄 How do I do a clean reinstall?</b></summary>

```bash
# Complete clean reinstall
clipwallet uninstall --purge
rm -rf ~/.clipwallet
sudo rm /usr/local/bin/clipwallet

# Then reinstall
curl -fsSL https://github.com/shaaravraghu/ClipWallet/releases/latest/download/install.sh | sh

# Or build from source
cargo build --release
sudo cp target/release/clipwallet /usr/local/bin/clipwallet
sudo codesign --sign - --force /usr/local/bin/clipwallet
clipwallet install
```
</details>

<details>
<summary><b>📝 Where are the logs located?</b></summary>

```bash
# Standard output (info logs)
~/.clipwallet/logs/out.log

# Error output
~/.clipwallet/logs/err.log

# Watch live logs
tail -f ~/.clipwallet/logs/out.log

# View last 50 lines
tail -50 ~/.clipwallet/logs/out.log
```
</details>

---

## Commands

| Command | Description |
|---------|-------------|
| `clipwallet run` | Start the service (foreground) |
| `clipwallet install` | Register as launchd login agent |
| `clipwallet uninstall` | Remove the launchd agent |
| `clipwallet uninstall --purge` | Remove agent + wipe all data + Keychain key |
| `clipwallet status` | Show daemon status, storage stats |
| `clipwallet change mode` | Interactively switch Static ↔ Dynamic |
| `clipwallet remove encryption` | Wipe vault + remove Keychain key |
| `clipwallet clear memory` | Erase all clipboard history from disk |
| `clipwallet vault-list` | List all encrypted vault entry IDs |
| `clipwallet vault-delete <id>` | Delete a specific vault entry |
| `clipwallet vault-rotate` | Rotate the encryption key (re-encrypts all entries) |

---

## Architecture

ClipWallet is built as a **layered system** with three storage tiers and five concurrent tasks.

### System Architecture

```
┌──────────────────────────────────────┐
│           Hotkey Layer               │
│  CGEventTap → ChordDetector → Engine│
├──────────────────────────────────────┤
│           Clipboard Layer            │
│  arboard + NSPasteboard (Obj-C FFI) │
├──────────────────────────────────────┤
│           Storage Layer              │
│  RAM → Disk (MessagePack) → Vault   │
│       (AES-256-GCM + Keychain)      │
└──────────────────────────────────────┘
```

### Concurrent Tasks

ClipWallet runs **5 concurrent tasks** to power the service:

| Task | Purpose | Technology |
|------|---------|-----------|
| **1. CGEventTap Grabber** | Intercepts and suppresses global hotkeys | Core Graphics (dedicated OS thread) |
| **2. Channel Bridge** | Bridges `std::sync` → `tokio` channels | Tokio async runtime |
| **3. Engine Event Loop** | Dispatches hotkey actions to clipboard operations | Tokio async runtime |
| **4. Periodic Flush** | Writes dirty RAM to disk every 60 seconds | Tokio interval timer |
| **5. Cursor Timeout Watchdog** | Auto-resets dynamic cursor after 10s idle | Tokio interval timer |

### Project Structure

```
ClipWallet/
├── src/
│   ├── main.rs              # Entry point, CLI, service orchestration
│   ├── config.rs            # Mode configuration (Static/Dynamic)
│   ├── engine.rs            # Core clipboard operation logic
│   ├── notify.rs            # macOS notification system
│   ├── static_store.rs      # Static mode (9 slots) implementation
│   │
│   ├── clipboard/           # Clipboard abstraction layer
│   │   ├── mod.rs           # Public API
│   │   ├── types.rs         # ClipEntry, ClipData types
│   │   ├── mime.rs          # MIME type detection
│   │   └── pasteboard.rs    # NSPasteboard Obj-C FFI
│   │
│   ├── hotkey/              # Hotkey interception system
│   │   ├── mod.rs           # Public API
│   │   ├── chord.rs         # Chord detection logic
│   │   ├── grabber.rs       # CGEventTap implementation
│   │   └── injector.rs      # Synthetic key injection
│   │
│   ├── storage/             # Three-tier storage system
│   │   ├── mod.rs           # Public API
│   │   ├── ram.rs           # In-memory store (RamStore)
│   │   ├── disk.rs          # MessagePack persistence
│   │   └── encrypted.rs     # AES-256-GCM vault + Keychain
│   │
│   └── daemon/              # macOS daemon management
│       ├── mod.rs           # Public API
│       ├── plist.rs         # launchd plist generation
│       └── status.rs        # Status reporting
│
├── assets/
│   └── com.clipwallet.agent.plist.template  # launchd template
│
├── Cargo.toml               # Rust dependencies
├── build_release.sh         # Release build script
├── install.sh               # Installation script
└── README.md                # This file
```

### Data Flow

```
User presses hotkey
       ↓
CGEventTap intercepts → ChordDetector evaluates
       ↓
HotkeyAction sent to Engine via channel
       ↓
Engine reads/writes clipboard via arboard/NSPasteboard
       ↓
Data stored in RAM (RamStore)
       ↓
Periodic flush writes to disk (MessagePack)
       ↓
Sensitive data encrypted to vault (AES-256-GCM)
       ↓
Encryption key stored in macOS Keychain
```

### Storage Tiers

| Tier | Technology | Purpose | Persistence |
|------|-----------|---------|-------------|
| **RAM** | In-memory HashMap | Fast access, active clipboard | Lost on restart |
| **Disk** | MessagePack serialization | Persistent storage | Survives reboots |
| **Vault** | AES-256-GCM encryption | Secure sensitive data | Encrypted at rest |

> **Note:** For a detailed technical deep-dive, see the source code in `src/`. Each module is well-documented with inline comments.

---

## Storage & Security

### Storage Locations

ClipWallet stores data in your home directory under `~/.clipwallet/`:

| Path | Contents | Format |
|------|----------|--------|
| `~/.clipwallet/config.toml` | Mode configuration (Static/Dynamic) | TOML |
| `~/.clipwallet/store/` | Clipboard persistence (unencrypted) | MessagePack |
| `~/.clipwallet/vault/` | Encrypted clipboard entries | AES-256-GCM |
| `~/.clipwallet/logs/` | Daemon stdout/stderr logs | Plain text |

### Encryption Details

ClipWallet uses **military-grade encryption** for sensitive clipboard data:

| Aspect | Details |
|--------|---------|
| **Algorithm** | AES-256-GCM (authenticated encryption) |
| **Key Size** | 256 bits (32 bytes) |
| **Key Storage** | macOS Keychain (`com.clipwallet.vault`) |
| **Key Generation** | Automatic on first run |
| **Nonce** | 12 bytes, randomly generated per entry |
| **File Format** | `[ 12-byte nonce ‖ ciphertext ‖ 16-byte auth tag ]` |

### Security Features

✅ **Encryption at rest** — Sensitive data encrypted on disk  
✅ **Authenticated encryption** — Prevents tampering (GCM mode)  
✅ **Secure key storage** — Keys stored in macOS Keychain, not on disk  
✅ **Automatic key generation** — No manual key management required  
✅ **Key rotation support** — Re-encrypt all entries with new key  
✅ **Local-only** — No network access, no data transmission  

### Key Management

```bash
# View encrypted entries
clipwallet vault-list

# Delete a specific encrypted entry
clipwallet vault-delete <id>

# Rotate encryption key (re-encrypts all entries)
clipwallet vault-rotate

# Remove encryption entirely
clipwallet remove encryption
```

### Privacy Guarantees

🔒 **Your data never leaves your Mac**
- No internet connection required
- No telemetry or analytics
- No cloud sync (unless you implement it)
- No third-party services

🔒 **You control your data**
- Full access to all files in `~/.clipwallet/`
- Open source — audit the code yourself
- Easy to export or delete all data

### Disk Space Usage

Typical disk usage:
- **Config:** <1 KB
- **Logs:** 1-10 MB (rotates automatically)
- **Clipboard history:** Depends on content (typically 5-50 MB)
- **Vault:** Depends on encrypted entries (typically 1-20 MB)

**Total:** Usually under 100 MB

---

## Supported Data Types

ClipWallet intelligently handles multiple clipboard data formats:

| Type | Detection | Storage | Notes |
|------|-----------|---------|-------|
| **Plain Text** | UTF-8 string | Stored inline as string | Most common type |
| **Rich Text (RTF)** | NSPasteboard RTF type | Raw RTF bytes | Preserves formatting |
| **Images** | PNG/JPEG/TIFF | Normalized to PNG bytes | Compressed for efficiency |
| **File Paths** | File URL pasteboard type | **Pointer-style** (path only) | Never stores file contents |
| **Binary Data** | Magic byte detection | Raw bytes with MIME type | Fallback for unknown types |

### How It Works

**Text:**
```
Copy: "Hello World" → Stored as UTF-8 string
Paste: UTF-8 string → System clipboard
```

**Images:**
```
Copy: Screenshot → Detected as PNG
      → Stored as PNG bytes
Paste: PNG bytes → System clipboard → Rendered in app
```

**Files:**
```
Copy: /Users/you/document.pdf → Stored as path string
Paste: Path → System clipboard → File reference (not contents!)
```

### Important Notes

⚠️ **File paths are pointers, not copies**
- Only the file path is stored, not the file contents
- If you move/delete the file, the clipboard entry becomes invalid
- This prevents RAM bloat from large files

✅ **Images are stored as bytes**
- Screenshots and copied images are fully stored
- No dependency on original file
- Normalized to PNG for consistency

✅ **Rich text preserves formatting**
- Bold, italic, colors, fonts preserved
- Stored as RTF (Rich Text Format)
- Compatible with most text editors

---

## Logs

### Log Locations

| Log File | Purpose | Location |
|----------|---------|----------|
| **Standard Output** | Info logs, operation logs | `~/.clipwallet/logs/out.log` |
| **Error Output** | Error logs, warnings | `~/.clipwallet/logs/err.log` |

### Viewing Logs

```bash
# Watch live logs (real-time)
tail -f ~/.clipwallet/logs/out.log

# View last 50 lines
tail -50 ~/.clipwallet/logs/out.log

# View error logs
cat ~/.clipwallet/logs/err.log

# Search logs for specific text
grep "error" ~/.clipwallet/logs/out.log

# View logs with timestamps
tail -f ~/.clipwallet/logs/out.log | while read line; do echo "$(date): $line"; done
```

### Log Levels

Set the `RUST_LOG` environment variable to control log verbosity:

```bash
# Error only
RUST_LOG=error clipwallet run

# Warning and above
RUST_LOG=warn clipwallet run

# Info and above (default)
RUST_LOG=info clipwallet run

# Debug (verbose)
RUST_LOG=debug clipwallet run

# Trace (very verbose)
RUST_LOG=trace clipwallet run
```

### What to Look For

**Normal operation:**
```
ClipWallet v0.1.0 starting in DYNAMIC mode...
Encryption key ready ✓
Disk state loaded into RAM ✓
ClipWallet running ✓  |  Mode: DYNAMIC  |  Listening for hotkeys...
```

**Hotkey activity:**
```
KeyDown KeyC  cmd=true opt=true
Action (press): DynamicCopy
Synced PlainText (42 chars)
```

**Errors to watch for:**
```
Failed to initialise encryption key: ...
Clipboard sync failed: ...
Periodic flush failed: ...
```

---

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (Edition 2021) |
| Async Runtime | Tokio |
| Key Interception | Core Graphics CGEventTap |
| Clipboard Access | arboard + NSPasteboard (Obj-C FFI) |
| Serialisation | MessagePack (rmp-serde) |
| Encryption | AES-256-GCM (aes-gcm crate) |
| Key Storage | macOS Keychain (keyring crate) |
| CLI | clap v4 |
| Logging | tracing + tracing-subscriber |
| Daemon | macOS launchd (plist) |

---

## Contributing

We welcome contributions! ClipWallet is part of **GirlScript Summer of Code (GSSoC) 2025**.

### How to Contribute

1. **Fork** the repository
2. **Create a branch** for your feature/fix:
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Make your changes** — follow the existing code style
4. **Test locally:**
   ```bash
   cargo build
   cargo run -- run
   ```
5. **Commit** with a descriptive message:
   ```bash
   git commit -m "feat: add XYZ functionality"
   ```
6. **Push** and open a **Pull Request**

### Contribution Guidelines

- Follow Rust conventions (`cargo fmt`, `cargo clippy`)
- Keep PRs focused — one feature or fix per PR
- Add comments for non-obvious logic
- Update documentation if your change affects user-facing behavior
- Reference the issue number in your PR description

### Areas for Contribution

<details>
<summary><b>📚 Documentation</b></summary>

- Improve README clarity and formatting
- Add code comments and inline documentation
- Create tutorials and guides
- Translate documentation to other languages
- Add diagrams and visual aids
</details>

<details>
<summary><b>🐛 Bug Fixes</b></summary>

- Fix reported issues
- Improve error handling
- Add validation and edge case handling
- Fix memory leaks or performance issues
</details>

<details>
<summary><b>✨ New Features</b></summary>

- Custom hotkey configuration
- Clipboard history search
- GUI overlay for visual slot selection
- Plugin system for clipboard transformations
- Cross-platform support (Windows, Linux)
</details>

<details>
<summary><b>🧪 Testing</b></summary>

- Add unit tests
- Add integration tests
- Test on different macOS versions
- Performance benchmarking
- Security audits
</details>

<details>
<summary><b>🎨 User Experience</b></summary>

- Improve installation process
- Better error messages
- Visual feedback for operations
- Accessibility improvements
</details>

### Good First Issues

Look for issues tagged with `good first issue` or `help wanted` in the [Issues](https://github.com/shaaravraghu/ClipWallet/issues) tab.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/shaaravraghu/ClipWallet.git
cd ClipWallet

# Install dependencies (Rust + Xcode CLI tools)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
xcode-select --install

# Build and run
cargo build
cargo run -- run

# Run tests (when available)
cargo test

# Check code quality
cargo fmt --check
cargo clippy
```

### Code Style

- Use `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Follow Rust naming conventions
- Add doc comments for public APIs
- Keep functions focused and small

### Commit Message Format

```
<type>: <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance tasks

**Examples:**
```
feat: add custom hotkey configuration
fix: resolve clipboard sync race condition
docs: improve installation instructions
```

### Contributors

See [CONTRIBUTORS.md](CONTRIBUTORS.md) for a list of people who have contributed to this project.

### Code of Conduct

Please read our [Code of Conduct](CONTRIBUTION_CODE_OF_CONDUCT.md) before contributing.

---

## Uninstall

### Remove Daemon Only

Removes the launchd agent but keeps all data:

```bash
clipwallet uninstall
```

You'll be prompted to optionally:
- Remove encryption key from Keychain
- Delete clipboard history

### Full Purge

Removes **everything** (daemon + data + encryption key):

```bash
clipwallet uninstall --purge
```

This will delete:
- launchd agent
- All clipboard history
- Encryption keys from Keychain
- Configuration files
- Log files

### Manual Cleanup

If you want to manually remove everything:

```bash
# Stop and remove daemon
clipwallet uninstall

# Remove binary
sudo rm /usr/local/bin/clipwallet

# Remove all data
rm -rf ~/.clipwallet

# Remove from Accessibility (manual)
# System Settings → Privacy & Security → Accessibility → Remove clipwallet
```

---

## Roadmap

### Planned Features

| Feature | Status | Target |
|---------|--------|--------|
| ✅ Multi-slot clipboard (Static mode) | **Completed** | v0.1.0 |
| ✅ Dynamic ring buffer mode | **Completed** | v0.1.0 |
| ✅ AES-256-GCM encryption | **Completed** | v0.1.0 |
| ✅ macOS Keychain integration | **Completed** | v0.1.0 |
| ✅ Rich data types (text, RTF, images, files) | **Completed** | v0.1.0 |
| 🔄 Windows support | In Progress | Jun 2026 |
| 🔄 Linux/Wayland support | In Progress | Jun 2026 |
| 📋 Clipboard history search | Planned | Jul 2026 |
| ⌨️ Custom hotkey configuration | Planned | Jul 2026 |
| 🌐 Clipboard sharing across devices | Planned | Future |
| 🎨 GUI overlay for visual slot selection | Planned | Future |
| 🔌 Plugin system for clipboard transformations | Planned | Future |

### Development Timeline

| Phase | Timeline | Focus |
|-------|----------|-------|
| **Phase 1** | May 15 - May 31, 2026 | Bug fixes, stability, deployment |
| **Phase 2** | Jun 01 - Jun 15, 2026 | Windows compatibility |
| **Phase 3** | Jun 15 - Jun 30, 2026 | Linux (Ubuntu) compatibility |
| **Phase 4** | Jul 01 - Aug 14, 2026 | Additional features & shortcuts |

### How to Request Features

1. Check existing [Issues](https://github.com/shaaravraghu/ClipWallet/issues) to avoid duplicates
2. Open a new issue with the `enhancement` label
3. Describe the feature and use case
4. Explain why it would be valuable

### How to Report Bugs

1. Check existing [Issues](https://github.com/shaaravraghu/ClipWallet/issues) to avoid duplicates
2. Open a new issue with the `bug` label
3. Include:
   - macOS version
   - ClipWallet version (`clipwallet --version`)
   - Steps to reproduce
   - Expected vs actual behavior
   - Relevant logs from `~/.clipwallet/logs/`

---

## License

This project is open source and available under the [MIT License](LICENSE).

---

<p align="center">
  Built with ❤️ in Rust — because your clipboard deserves better.
</p>
