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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub mode: ClipMode,
    #[serde(default = "default_ring_capacity")]
    pub ring_capacity: usize,
    /// Entries whose payload is at least this many bytes are spilled to disk and
    /// held in RAM only as a lightweight pointer until pasted (issue #28).
    /// Set to `0` to disable spilling and keep all entries fully resident.
    #[serde(default = "default_spill_threshold")]
    pub spill_threshold_bytes: usize,
}

fn default_ring_capacity() -> usize {
    50
}

/// 1 MiB — large enough that text snippets and small icons stay resident, low
/// enough that screenshots and binary blobs spill to disk.
fn default_spill_threshold() -> usize {
    1024 * 1024
}

/// Spilling is disabled at `0`; otherwise the threshold is clamped to a sane
/// band so a malformed config can't spill on every keystroke or never at all.
const SPILL_THRESHOLD_MIN: usize = 4 * 1024; // 4 KiB
const SPILL_THRESHOLD_MAX: usize = 256 * 1024 * 1024; // 256 MiB

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: ClipMode::Dynamic,
            ring_capacity: 50,
            spill_threshold_bytes: default_spill_threshold(),
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
            Ok(s)  => toml::from_str(&s).unwrap_or_else(|_| Config::default()),
            Err(_) => Config::default(),
        }
    };

    cfg.ring_capacity = cfg.ring_capacity.clamp(10, 500);
    if cfg.spill_threshold_bytes != 0 {
        cfg.spill_threshold_bytes = cfg
            .spill_threshold_bytes
            .clamp(SPILL_THRESHOLD_MIN, SPILL_THRESHOLD_MAX);
    }
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_threshold_is_one_mib() {
        assert_eq!(Config::default().spill_threshold_bytes, 1024 * 1024);
    }

    #[test]
    fn legacy_config_without_threshold_field_deserialises() {
        // Configs written before issue #28 have no spill_threshold_bytes key.
        let toml_str = "mode = \"dynamic\"\nring_capacity = 50\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.spill_threshold_bytes, default_spill_threshold());
    }

    #[test]
    fn threshold_zero_disables_and_is_not_clamped() {
        let toml_str = "mode = \"dynamic\"\nspill_threshold_bytes = 0\n";
        let mut cfg: Config = toml::from_str(toml_str).unwrap();
        // mirror the clamping load() applies
        if cfg.spill_threshold_bytes != 0 {
            cfg.spill_threshold_bytes = cfg
                .spill_threshold_bytes
                .clamp(SPILL_THRESHOLD_MIN, SPILL_THRESHOLD_MAX);
        }
        assert_eq!(cfg.spill_threshold_bytes, 0);
    }

    #[test]
    fn tiny_threshold_is_clamped_up() {
        let mut v = 16usize;
        if v != 0 {
            v = v.clamp(SPILL_THRESHOLD_MIN, SPILL_THRESHOLD_MAX);
        }
        assert_eq!(v, SPILL_THRESHOLD_MIN);
    }
}
