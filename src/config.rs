use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub show_system_components: bool,
    pub default_sort: SortOrder,
    pub scan_store: bool,
    pub scan_portable: bool,
    pub scan_steam: bool,
    pub scan_chocolatey: bool,
    pub portable_scan_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Name,
    Version,
    Source,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_system_components: false,
            default_sort: SortOrder::Name,
            scan_store: true,
            scan_portable: true,
            scan_steam: true,
            scan_chocolatey: true,
            portable_scan_paths: vec![
                r"C:\Program Files".into(),
                r"C:\Program Files (x86)".into(),
            ],
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let appdata = env::var("APPDATA").context("APPDATA environment variable is not set")?;
        Ok(PathBuf::from(appdata).join("clirm"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&contents).context("failed to parse config.toml")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let contents = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert!(!config.show_system_components);
        assert_eq!(config.default_sort, SortOrder::Name);
        assert!(config.scan_store);
    }

    #[test]
    fn config_round_trip_toml() {
        let config = Config {
            show_system_components: true,
            default_sort: SortOrder::Version,
            scan_store: false,
            scan_portable: true,
            scan_steam: false,
            scan_chocolatey: true,
            portable_scan_paths: vec![r"D:\Portable".into()],
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, config);
    }
}