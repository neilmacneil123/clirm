pub mod registry;

use crate::config::{Config, SortOrder};
use crate::models::ApplicationEntry;
use anyhow::Result;

pub fn scan_applications(config: &Config) -> Result<Vec<ApplicationEntry>> {
    let mut apps = registry::scan_registry()?;

    if !config.show_system_components {
        apps.retain(|app| !app.is_system_component);
    }

    sort_applications(&mut apps, config.default_sort);
    Ok(apps)
}

pub fn sort_applications(apps: &mut [ApplicationEntry], order: SortOrder) {
    match order {
        SortOrder::Name => apps.sort_by_key(|a| a.display_name.to_lowercase()),
        SortOrder::Version => apps.sort_by(|a, b| {
            a.version
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&b.version.as_deref().unwrap_or("").to_lowercase())
        }),
        SortOrder::Source => apps.sort_by(|a, b| {
            a.source
                .label()
                .cmp(b.source.label())
                .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
        }),
    }
}