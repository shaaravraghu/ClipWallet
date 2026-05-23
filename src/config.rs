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

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mode: ClipMode,
    pub ring_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: ClipMode::Dynamic,
            ring_capacity: 50,
        }
    }
}

pub const MIN_RING_CAPACITY: usize = 10;
pub const MAX_RING_CAPACITY: usize = 1000;

pub fn validate_capacity(cap: usize) -> usize {
    cap.clamp(
        MIN_RING_CAPACITY,
        MAX_RING_CAPACITY,
    )
}

fn config_path() -> PathBuf {
    home_dir()
        .expect("No home dir")
        .join(".clipwallet")
        .join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();

    if !path.exists() {
        let mut cfg = Config::default();

        cfg.ring_capacity =
            validate_capacity(cfg.ring_capacity);

        let _ = save(&cfg);

        return cfg;
    }

    match fs::read_to_string(&path) {
        Ok(s) => {
            let mut cfg =
                toml::from_str(&s)
                    .unwrap_or_else(|_| Config::default());

            cfg.ring_capacity =
                validate_capacity(cfg.ring_capacity);

            cfg
        }

        Err(_) => Config::default(),
    }
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