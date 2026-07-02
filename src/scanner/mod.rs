pub mod chocolatey;
pub mod features;
pub mod portable;
pub mod registry;
pub mod steam;
pub mod store;
pub mod updates;

use crate::config::{Config, SortOrder};
use crate::models::ApplicationEntry;
use crate::protection::ProtectionList;
use crate::ratings;
use anyhow::Result;

pub fn scan_applications(
    config: &Config,
    protection: &ProtectionList,
) -> Result<Vec<ApplicationEntry>> {
    let mut apps = registry::scan_registry()?;

    if config.scan_store {
        match store::scan_store_apps() {
            Ok(mut store_apps) => apps.append(&mut store_apps),
            Err(e) => tracing::warn!("store scan failed: {e}"),
        }
    }

    if config.scan_portable {
        match portable::scan_portable(config) {
            Ok(mut portable_apps) => apps.append(&mut portable_apps),
            Err(e) => tracing::warn!("portable scan failed: {e}"),
        }
    }

    if config.scan_steam {
        match steam::scan_steam() {
            Ok(mut steam_apps) => apps.append(&mut steam_apps),
            Err(e) => tracing::warn!("steam scan failed: {e}"),
        }
    }

    if config.scan_chocolatey {
        match chocolatey::scan_chocolatey() {
            Ok(mut choco_apps) => apps.append(&mut choco_apps),
            Err(e) => tracing::warn!("chocolatey scan failed: {e}"),
        }
    }

    if config.scan_features {
        match features::scan_windows_features() {
            Ok(mut feature_apps) => apps.append(&mut feature_apps),
            Err(e) => tracing::warn!("windows features scan failed: {e}"),
        }
    }

    if config.scan_updates {
        match updates::scan_windows_updates() {
            Ok(mut update_apps) => apps.append(&mut update_apps),
            Err(e) => tracing::warn!("windows updates scan failed: {e}"),
        }
    }

    if !config.show_system_components {
        apps.retain(|app| !app.is_system_component);
    }

    for app in &mut apps {
        if protection.is_protected(&app.id) {
            app.is_protected = true;
        }
    }

    ratings::apply_ratings(&mut apps);
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
            a.source.label().cmp(b.source.label()).then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
        }),
    }
}
