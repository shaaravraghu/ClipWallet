use dirs::home_dir;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

const PLIST_LABEL: &str = "com.clipwallet.agent";
const INSTALL_PATH: &str = "/usr/local/bin/clipwallet";

fn plist_path() -> anyhow::Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("No home dir found"))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", PLIST_LABEL)))
}

pub fn install() -> anyhow::Result<()> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("No home dir found"))?;
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
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>

    <key>ThrottleInterval</key>
    <integer>5</integer>

    <key>StandardOutPath</key>
    <string>{home}/.clipwallet/logs/out.log</string>

    <key>StandardErrorPath</key>
    <string>{home}/.clipwallet/logs/err.log</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
</dict>
</plist>"#,
        label  = PLIST_LABEL,
        binary = INSTALL_PATH,
        home   = home.to_string_lossy(),
    );

    let path = plist_path()?;
    let parent = path.parent().ok_or_else(|| anyhow::anyhow!("Invalid plist path"))?;
    fs::create_dir_all(parent)?;
    fs::write(&path, &plist)?;
    info!("Plist written → {:?}", path);

    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid unicode in plist path"))?;

    // Unload first in case an old version is running
    let _ = Command::new("launchctl")
        .args(["unload", path_str])
        .output();

    Command::new("launchctl")
        .args(["load", "-w", path_str])
        .status()?;

    info!("ClipWallet daemon registered with launchd ✓");
    println!("ClipWallet daemon registered with launchd ✓");
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let path = plist_path()?;
    if path.exists() {
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid unicode in plist path"))?;
        let _ = Command::new("launchctl")
            .args(["unload", path_str])
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