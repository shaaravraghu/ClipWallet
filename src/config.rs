use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipMode {
    Static,
    Dynamic,
}

impl std::fmt::Display for ClipMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipMode::Static  => write!(f, "static"),
            ClipMode::Dynamic => write!(f, "dynamic"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mode: ClipMode,
}

impl Default for Config {
    fn default() -> Self {
        Self { mode: ClipMode::Dynamic }
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("No home dir found"))?;
    Ok(home.join(".clipwallet").join("config.toml"))
}

pub fn load() -> Config {
    let path = match config_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Cannot determine config path: {}", e);
            return Config::default();
        }
    };
    if !path.exists() {
        let cfg = Config::default();
        let _ = save(&cfg);
        return cfg;
    }
    match fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_else(|_| Config::default()),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> anyhow::Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = toml::to_string(cfg)?;
    fs::write(&path, s)?;
    Ok(())
}

pub fn set_mode(mode: ClipMode) -> anyhow::Result<()> {
    let mut cfg = load();
    cfg.mode = mode.clone();
    save(&cfg)?;
    info!("Mode set to: {}", mode);
    println!("ClipWallet mode set to: {} ✓", mode);
    println!("Restart the daemon for changes to take effect:");
    println!("  clipwallet uninstall && clipwallet install");
    Ok(())
}