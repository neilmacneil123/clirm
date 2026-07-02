use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;

use crate::config::Config;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ProtectionFile {
    protected_ids: Vec<String>,
}

pub struct ProtectionList {
    ids: HashSet<String>,
    path: std::path::PathBuf,
}

impl ProtectionList {
    pub fn load() -> Result<Self> {
        let path = Config::config_dir()?.join("protected.toml");
        let ids = if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let file: ProtectionFile =
                toml::from_str(&contents).context("failed to parse protected.toml")?;
            file.protected_ids.into_iter().collect()
        } else {
            HashSet::new()
        };

        Ok(Self { ids, path })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut ids: Vec<_> = self.ids.iter().cloned().collect();
        ids.sort();
        let file = ProtectionFile { protected_ids: ids };
        let contents = toml::to_string_pretty(&file)?;
        fs::write(&self.path, contents)?;
        Ok(())
    }

    pub fn is_protected(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn toggle(&mut self, id: &str) -> bool {
        if self.ids.contains(id) {
            self.ids.remove(id);
            false
        } else {
            self.ids.insert(id.to_string());
            true
        }
    }
}
