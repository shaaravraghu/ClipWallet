//! Daemon auto-start registration.
//! macOS: launchd plist. Linux: stub.

// ── macOS: launchd ────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use dirs::home_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use tracing::info;

    const PLIST_LABEL: &str = "com.clipwallet.agent";
    const INSTALL_PATH: &str = "/usr/local/bin/clipwallet";

    fn plist_path() -> PathBuf {
        home_dir()
            .expect("No home dir")
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", PLIST_LABEL))
    }

    pub fn install() -> anyhow::Result<()> {
        let home = home_dir().expect("No home dir");
        let log_dir = home.join(".clipwallet").join("logs");
        fs::create_dir_all(&log_dir)?;

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict><key>SuccessfulExit</key><false/></dict>
    <key>ThrottleInterval</key>
    <integer>5</integer>
    <key>StandardOutPath</key>
    <string>{home}/.clipwallet/logs/out.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/.clipwallet/logs/err.log</string>
    <key>EnvironmentVariables</key>
    <dict><key>RUST_LOG</key><string>info</string></dict>
</dict>
</plist>"#,
            label  = PLIST_LABEL,
            binary = INSTALL_PATH,
            home   = home.to_string_lossy(),
        );

        let path = plist_path();
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &plist)?;
        info!("Plist written → {:?}", path);

        let _ = Command::new("launchctl")
            .args(["unload", path.to_str().unwrap()])
            .output();
        Command::new("launchctl")
            .args(["load", "-w", path.to_str().unwrap()])
            .status()?;

        info!("ClipWallet daemon registered with launchd ✓");
        println!("ClipWallet daemon registered with launchd ✓");
        Ok(())
    }

    pub fn uninstall() -> anyhow::Result<()> {
        let path = plist_path();
        if path.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", path.to_str().unwrap()])
                .output();
            fs::remove_file(&path)?;
            info!("ClipWallet daemon removed from launchd ✓");
            println!("ClipWallet daemon removed from launchd ✓");
        } else {
            println!("No launchd agent found — already uninstalled.");
        }
        Ok(())
    }

    pub fn status() {
        println!("── launchd status ───────────────────────────────");
        let _ = Command::new("launchctl")
            .args(["list", PLIST_LABEL])
            .status();
    }
}

// ── Linux: stub ────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn install() -> anyhow::Result<()> {
        println!("Install not supported on this platform — run `clipwallet run` manually.");
        Ok(())
    }

    pub fn uninstall() -> anyhow::Result<()> {
        println!("Uninstall not supported on this platform.");
        Ok(())
    }

    pub fn status() {
        println!("── Daemon status ────────────────────────────────");
        println!("  Auto-start  : Not supported on this platform.");
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub use platform::{install, status, uninstall};