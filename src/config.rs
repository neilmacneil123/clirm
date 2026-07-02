use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub show_system_components: bool,
    pub default_sort: SortOrder,
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
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let appdata = env::var("APPDATA").context("APPDATA environment variable is not set")?;
        Ok(PathBuf::from(appdata).join("clirm").join("config.toml"))
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
    }

    #[test]
    fn config_round_trip_toml() {
        let config = Config {
            show_system_components: true,
            default_sort: SortOrder::Version,
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn config_path_uses_appdata_clirm() {
        let path = Config::config_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.ends_with(r"clirm\config.toml") || path_str.ends_with("clirm/config.toml"));
    }
}