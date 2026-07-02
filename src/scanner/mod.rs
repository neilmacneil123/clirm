pub mod registry;

use crate::config::Config;
use crate::models::ApplicationEntry;
use anyhow::Result;

pub fn scan_applications(config: &Config) -> Result<Vec<ApplicationEntry>> {
    let mut apps = registry::scan_registry()?;

    if !config.show_system_components {
        apps.retain(|app| !app.is_system_component);
    }

    Ok(apps)
}
