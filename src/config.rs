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
            ClipMode::Static => write!(f, "static"),
            ClipMode::Dynamic => write!(f, "dynamic"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub mode: ClipMode,
    #[serde(default = "default_ring_capacity")]
    pub ring_capacity: usize,
}

fn default_ring_capacity() -> usize {
    50
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: ClipMode::Dynamic,
            ring_capacity: 50,
        }
    }
}

fn config_path() -> PathBuf {
    home_dir()
        .expect("No home dir")
        .join(".clipwallet")
        .join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();

    let mut cfg = if !path.exists() {
        let cfg = Config::default();
        let _ = save(&cfg);
        cfg
    } else {
        match fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|_| Config::default()),
            Err(_) => Config::default(),
        }
    };

    cfg.ring_capacity = cfg.ring_capacity.clamp(10, 500);
    cfg
}

pub fn save(cfg: &Config) -> anyhow::Result<()> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap())?;
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
