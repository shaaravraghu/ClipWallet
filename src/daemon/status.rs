use crate::storage::disk::store_dir;
use crate::storage::encrypted::vault_dir;
use crate::storage::spill::blob_dir;
use dirs::home_dir;
use std::fs;
use std::process::Command;

/// Print a full human-readable status report
pub fn print_full_status() {
    println!("── ClipWallet Status ────────────────────────────");

    // ── Process ───────────────────────────────────────────────────────
    let output = Command::new("pgrep")
        .args(["-x", "clipwallet"])
        .output();

    match output {
        Ok(o) if !o.stdout.is_empty() => {
            let pid = String::from_utf8_lossy(&o.stdout).trim().to_string();
            println!("  Process   : Running (PID {})", pid);
        }
        _ => println!("  Process   : Not running"),
    }

    // ── launchd ───────────────────────────────────────────────────────
    let plist = home_dir()
        .unwrap()
        .join("Library/LaunchAgents/com.clipwallet.agent.plist");
    println!(
        "  launchd   : {}",
        if plist.exists() { "Registered ✓" } else { "Not registered" }
    );

    // ── Store stats ───────────────────────────────────────────────────
    let store = store_dir();
    let static_count = (1..=9)
        .filter(|i| store.join(format!("slot_{}.mpk", i)).exists())
        .count();
    let ring_path = store.join("dynamic_ring.mpk");
    let ring_size = if ring_path.exists() {
        fs::metadata(&ring_path)
            .map(|m| format!("{} bytes", m.len()))
            .unwrap_or_else(|_| "unknown".into())
    } else {
        "empty".into()
    };

    println!("  Static    : {}/9 slots occupied", static_count);
    println!("  Ring      : {} on disk", ring_size);

    // ── Spilled blob stats ────────────────────────────────────────────
    let blobs = blob_dir();
    let (blob_count, blob_bytes) = if blobs.exists() {
        fs::read_dir(&blobs)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("blob"))
                    .fold((0usize, 0u64), |(n, sz), e| {
                        let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                        (n + 1, sz + len)
                    })
            })
            .unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    println!(
        "  Blobs     : {} large entr{} on disk ({} bytes)",
        blob_count,
        if blob_count == 1 { "y" } else { "ies" },
        blob_bytes
    );

    // ── Vault stats ───────────────────────────────────────────────────
    let vault = vault_dir();
    let vault_count = if vault.exists() {
        fs::read_dir(&vault)
            .map(|d| d.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    } else {
        0
    };
    println!("  Vault     : {} encrypted entries", vault_count);

    // ── Log paths ─────────────────────────────────────────────────────
    let log_dir = home_dir().unwrap().join(".clipwallet/logs");
    println!("  Logs      : {}", log_dir.display());
    println!("  Store     : {}", store.display());
    println!("  Vault     : {}", vault.display());
    println!("─────────────────────────────────────────────────");
}

/// Wipe all ClipWallet data from disk (called on full uninstall)
pub fn wipe_all_data() -> anyhow::Result<()> {
    let base = home_dir().unwrap().join(".clipwallet");
    if base.exists() {
        fs::remove_dir_all(&base)?;
        println!("All ClipWallet data removed from disk ✓");
    }
    Ok(())
}