mod clipboard;
mod config;
mod daemon;
mod engine;
mod hotkey;
mod notify;
mod storage;

use crate::config::Config;
use crate::config::{set_mode, ClipMode};
use crate::engine::Engine;
use crate::hotkey::HotkeyAction;
use crate::storage::{disk, ram::FLUSH_INTERVAL_SECS, RamStore};
use clap::{Parser, Subcommand};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "clipwallet",
    version = "0.1.0",
    about = "A persistent, encrypted clipboard manager for macOS",
    long_about = "
ClipWallet — Multi-slot clipboard manager with RAM/disk/encrypted storage.

MODES:
  Static  — 9 named slots (Cmd+Opt+C/V/X + digit 1-9)
  Dynamic — Recency-ordered ring of up to 50 entries

Encryption and memory are always ON by default.
  clipwallet remove encryption   — wipes vault + Keychain key
  clipwallet clear memory        — erases all clipboard history from disk
  clipwallet change mode         — interactively switch Static ↔ Dynamic
"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the ClipWallet background service (foreground)
    Run {
        /// Override the dynamic-ring capacity (entries kept in RAM).
        /// Defaults to the configured ring capacity (50).
        #[arg(long, value_name = "ENTRIES")]
        ring_capacity: Option<usize>,
    },

    /// Install ClipWallet as a launchd login agent (auto-starts on login)
    Install,

    /// Uninstall the launchd agent
    Uninstall {
        /// Also wipe all stored data and encryption key
        #[arg(long, default_value_t = false)]
        purge: bool,
    },

    /// Show daemon and storage status
    Status,

    /// Interactively switch between Static and Dynamic mode
    #[command(name = "change")]
    Change {
        #[command(subcommand)]
        what: ChangeTarget,
    },

    /// Remove vault encryption and wipe the Keychain key
    #[command(name = "remove")]
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },

    /// Clear all clipboard history from disk and RAM
    #[command(name = "clear")]
    Clear {
        #[command(subcommand)]
        target: ClearTarget,
    },

    /// List all encrypted vault entries
    VaultList,

    /// Delete an encrypted vault entry by ID
    VaultDelete {
        #[arg(help = "Entry ID to delete from vault")]
        id: u64,
    },

    /// Rotate the vault encryption key
    VaultRotate,
}

#[derive(Subcommand)]
enum ChangeTarget {
    /// Switch between Static and Dynamic mode
    Mode,
}

#[derive(Subcommand)]
enum RemoveTarget {
    /// Remove vault encryption and wipe the Keychain key
    Encryption,
}

#[derive(Subcommand)]
enum ClearTarget {
    /// Erase all clipboard history (dynamic ring + static slots) from disk
    Memory,
}

// ── Entry Point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { ring_capacity } => run_service(ring_capacity).await?,

        // ── Install / Uninstall ───────────────────────────────────────────────
        Commands::Install => {
            daemon::plist::install()?;
            println!("ClipWallet installed ✓ — will auto-start on every login.");
            println!("Encryption and memory are ON by default.");
            println!("To switch mode:        clipwallet change mode");
            println!("To remove encryption:  clipwallet remove encryption");
            println!("To clear history:      clipwallet clear memory");
        }

        Commands::Uninstall { purge } => {
            daemon::plist::uninstall()?;

            if purge {
                storage::delete_key()?;
                daemon::status::wipe_all_data()?;
                println!("ClipWallet fully purged ✓");
            } else {
                println!("Also remove encryption key from Keychain? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim().eq_ignore_ascii_case("y") {
                    if let Err(e) = storage::delete_key() {
                        eprintln!("Warning: Could not remove key: {}", e);
                    } else {
                        println!("Keychain key removed ✓");
                    }
                }

                println!("Also delete all stored clipboard data? [y/N]");
                let mut input2 = String::new();
                std::io::stdin().read_line(&mut input2)?;
                if input2.trim().eq_ignore_ascii_case("y") {
                    daemon::status::wipe_all_data()?;
                    println!("Clipboard data wiped ✓");
                }
            }
        }

        // ── change mode ───────────────────────────────────────────────────────
        Commands::Change {
            what: ChangeTarget::Mode,
        } => {
            let current = config::load().mode;
            println!("Current mode: {}", current);
            println!();
            println!("Choose new mode:");
            println!("  1 — Static  (Cmd+Opt+C/V/X + digit 1-9, 9 named slots)");
            println!("  2 — Dynamic (Cmd+Opt+C, Tab to navigate ring)");
            println!();
            print!("Enter 1 or 2: ");

            use std::io::Write;
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            let new_mode = match input.trim() {
                "1" => ClipMode::Static,
                "2" => ClipMode::Dynamic,
                _ => {
                    println!("Invalid choice — mode unchanged.");
                    return Ok(());
                }
            };

            if new_mode == current {
                println!("Already in {} mode — no change needed.", current);
                return Ok(());
            }

            set_mode(new_mode.clone())?;
            notify::notify_mode_changed(&new_mode.to_string());
            println!();
            println!("Mode changed to: {} ✓", new_mode);
            println!("Restart the daemon to apply:");
            println!("  clipwallet uninstall && clipwallet install");
        }

        // ── remove encryption ─────────────────────────────────────────────────
        Commands::Remove {
            target: RemoveTarget::Encryption,
        } => {
            println!("This will permanently delete the encryption key from Keychain");
            println!("and erase all vault-encrypted entries. Continue? [y/N]");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted — encryption unchanged.");
                return Ok(());
            }

            // Wipe vault files
            let vault_dir = storage::encrypted::vault_dir();
            if vault_dir.exists() {
                std::fs::remove_dir_all(&vault_dir)?;
                println!("Vault entries deleted ✓");
            } else {
                println!("No vault entries found.");
            }

            // Remove Keychain key
            match storage::delete_key() {
                Ok(_) => println!("Keychain key removed ✓"),
                Err(e) => println!("Keychain key not found ({})", e),
            }

            notify::notify_encryption_removed();
            println!("Encryption removed. New data will still be stored (unencrypted vault).");
            println!(
                "To re-enable: restart the daemon — it will generate a new key automatically."
            );
        }

        // ── clear memory ──────────────────────────────────────────────────────
        Commands::Clear {
            target: ClearTarget::Memory,
        } => {
            println!("This will erase ALL clipboard history (dynamic ring + static slots).");
            println!("Vault-encrypted entries are NOT affected. Continue? [y/N]");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted — memory unchanged.");
                return Ok(());
            }

            daemon::status::wipe_all_data()?;
            notify::notify_memory_cleared();
            println!("Memory cleared ✓ — clipboard history erased from disk.");
            println!("Restart the daemon to clear RAM too:");
            println!("  clipwallet uninstall && clipwallet install");
        }

        // ── Status ────────────────────────────────────────────────────────────
        Commands::Status => {
            daemon::plist::status();
            daemon::status::print_full_status();
        }

        // ── Vault ─────────────────────────────────────────────────────────────
        Commands::VaultList => {
            let ids = storage::list_vault_ids();
            if ids.is_empty() {
                println!("Vault is empty.");
            } else {
                println!("── Vault Entries ────────────────────────────────────");
                for id in &ids {
                    println!("  id = {}", id);
                }
                println!("  Total: {}", ids.len());
                println!("─────────────────────────────────────────────────────");
            }
        }

        Commands::VaultDelete { id } => {
            storage::delete_from_vault(id)?;
            println!("Vault entry {} deleted ✓", id);
        }

        Commands::VaultRotate => rotate_vault_key()?,
    }

    Ok(())
}

// ── Service Loop ──────────────────────────────────────────────────────────────

async fn run_service(ring_capacity_override: Option<usize>) -> anyhow::Result<()> {
    // ── Load mode from persistent config ─────────────────────────────────────
    let cfg = config::load();
    let mode_static = cfg.mode == ClipMode::Static;

    // ── Determine ring capacity ───────────────────────────────────────────────
    let capacity = ring_capacity_override
        .unwrap_or(cfg.ring_capacity)
        .clamp(10, 500);
    info!(
        "ClipWallet v0.1.0 starting in {} mode | ring capacity = {}...",
        if mode_static { "STATIC" } else { "DYNAMIC" },
        capacity
    );

    // ── Encryption + memory ON by default ────────────────────────────────────
    // The vault key is created automatically if it doesn't exist.
    // No user action required — it just works.
    if let Err(e) = storage::encrypted::get_or_create_key() {
        error!("Failed to initialise encryption key: {}", e);
    } else {
        info!("Encryption key ready ✓");
    }

    // ── Shared RAM store ──────────────────────────────────────────────────────
    let ram = Arc::new(RwLock::new(RamStore::new(capacity)));
    {
        let mut w = ram.write().unwrap();
        disk::cleanup_tmp_files();
        if let Err(e) = disk::load(&mut w) {
            error!("Disk load failed: {}", e);
        }
    }
    info!("Disk state loaded into RAM ✓");

    // ── Shared tokio channel ──────────────────────────────────────────────────
    let (tx, mut rx) = mpsc::unbounded_channel::<HotkeyAction>();

    // ── Task 1: CGEventTap key grabber ────────────────────────────────────────
    let (tap_tx, tap_rx) = std::sync::mpsc::sync_channel::<HotkeyAction>(64);
    let tx_bridge = tx.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(action) = tap_rx.recv() {
            let _ = tx_bridge.send(action);
        }
    });
    // Returns a handle we can use from the shutdown handler to stop the tap cleanly
    // via CFRunLoopStop instead of leaving it running while
    // we flush: closes the race where late events would mutate RAM
    // mid-flush.
    let grabber = hotkey::grabber::spawn_event_tap(tap_tx, mode_static);

    // ── Task 2: Engine event loop ─────────────────────────────────────────────
    let ram_engine = Arc::clone(&ram);
    let mut engine = Engine::new(ram_engine)?;
    let engine_handle = tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            engine.handle(action);
        }
    });

    // ── Task 3: Periodic flush (every 60 seconds) ─────────────────────────────
    // Interval and rationale documented in storage/ram.rs as
    // FLUSH_INTERVAL_SECS. This is the only protection against data loss
    // from SIGKILL / kill -9 / kernel panic / hard power-off, since those
    // bypass the ctrlc handler below.
    let ram_flush = Arc::clone(&ram);
    let flush_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            let mut w = ram_flush.write().unwrap();
            if let Err(e) = disk::flush(&mut w) {
                error!("Periodic flush failed: {}", e);
            }
        }
    });

    // ── Task 4: Signal handler (Ctrl+C / SIGTERM → clean shutdown) ────────────
    // ctrlc's "termination" feature in Cargo.toml means this handler catches
    // both SIGINT and SIGTERM. Do NOT add a tokio::signal::unix handler in
    // parallel: they will race for the same signals.
    let ram_signal = Arc::clone(&ram);
    let grabber_signal = grabber.clone();
    let shutting_down = Arc::new(std::sync::atomic::AtomicBool::new(false));
    ctrlc::set_handler(move || {
        // Re-entry guard: launchd can fire SIGTERM more than once during a
        // forced logout, and a fast double Ctrl+C will also re-enter.
        // Without this, the second flush races the first and can corrupt
        // the on-disk store.
        if shutting_down.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        info!("Shutdown signal — flushing to disk...");

        // 1. Stop the CGEventTap thread BEFORE flushing so no new actions
        //    can mutate RAM while we serialize it.
        grabber_signal.stop();

        // 2. Force-flush regardless of the dirty flag. We don't trust the
        //    flag at shutdown: if something is in RAM that isn't on disk,
        //    it doesn't matter whether the flag was set correctly.
        let mut w = ram_signal.write().unwrap();
        if let Err(e) = disk::flush_force(&mut w) {
            error!("Final flush failed: {}", e);
        }
        info!("ClipWallet stopped cleanly ✓");
        std::process::exit(0);
    })
    .expect("Cannot set signal handler");

    // ── Task 5: Cursor timeout watchdog ───────────────────────────────────────
    let ram_cursor = Arc::clone(&ram);
    let tx_cursor = tx.clone();
    let cursor_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            if ram_cursor.read().unwrap().cursor_timed_out() {
                let _ = tx_cursor.send(HotkeyAction::CursorReset);
            }
        }
    });

    info!(
        "ClipWallet running ✓  |  Mode: {}  |  Listening for hotkeys...",
        if mode_static { "STATIC" } else { "DYNAMIC" }
    );
    info!("Tip: System Settings → Privacy & Security → Accessibility → add clipwallet");

    let _ = tokio::join!(engine_handle, flush_handle, cursor_handle);
    Ok(())
}

// ── Vault Key Rotation ────────────────────────────────────────────────────────

fn rotate_vault_key() -> anyhow::Result<()> {
    use storage::encrypted::{
        decrypt_entry, delete_key, get_or_create_key, save_to_vault, vault_dir,
    };

    println!("Rotating encryption key...");
    let dir = vault_dir();
    if !dir.exists() {
        println!("No vault entries found — nothing to rotate.");
        return Ok(());
    }

    let ids = storage::list_vault_ids();
    let mut entries = Vec::new();
    for id in &ids {
        let path = dir.join(format!("{}.vlt", id));
        let bytes = std::fs::read(&path)?;
        match decrypt_entry(&bytes) {
            Ok(e) => entries.push(e),
            Err(e) => eprintln!("Warning: Could not decrypt id={}: {}", id, e),
        }
    }

    delete_key()?;
    get_or_create_key()?;
    for entry in &entries {
        save_to_vault(entry)?;
    }
    println!("Key rotated — {} entries re-encrypted ✓", entries.len());
    Ok(())
}
