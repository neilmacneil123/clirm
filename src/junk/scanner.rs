use crate::models::{ApplicationEntry, JunkItem, JunkKind};
use crate::path_safety::validate_deletable_path;
use crate::registry_util::{is_deletable_registry_path, parse_registry_path, RegistryHive};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const MAX_REGISTRY_SCAN_DEPTH: usize = 8;

pub fn scan_for_app(app: &ApplicationEntry) -> Result<Vec<JunkItem>> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Some(location) = &app.install_location {
        collect_path_leftovers(
            &mut items,
            &mut seen,
            Path::new(location.trim()),
            &app.display_name,
        );
    }

    collect_appdata_leftovers(&mut items, &mut seen, app)?;
    collect_registry_leftovers(&mut items, &mut seen, app)?;

    Ok(items)
}

pub fn scan_orphan_leftovers(apps: &[ApplicationEntry]) -> Result<Vec<JunkItem>> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for app in apps.iter().filter(|a| a.is_orphan) {
        if let Some(location) = &app.install_location {
            collect_path_leftovers(
                &mut items,
                &mut seen,
                Path::new(location.trim()),
                &app.display_name,
            );
        }
        collect_appdata_leftovers(&mut items, &mut seen, app)?;
    }

    Ok(items)
}

pub fn delete_junk_item(item: &JunkItem) -> Result<()> {
    match item.kind {
        JunkKind::File | JunkKind::Directory => {
            validate_deletable_path(Path::new(&item.path))?;
            if Path::new(&item.path).exists() {
                if item.kind == JunkKind::Directory {
                    fs::remove_dir_all(&item.path)
                        .with_context(|| format!("failed to delete directory {}", item.path))?;
                } else {
                    fs::remove_file(&item.path)
                        .with_context(|| format!("failed to delete file {}", item.path))?;
                }
            }
        }
        JunkKind::RegistryKey => {
            if !is_deletable_registry_path(&item.path) {
                anyhow::bail!("refusing to delete unsafe registry path: {}", item.path);
            }
            delete_registry_key(&item.path)?;
        }
    }

    tracing::info!("deleted leftover: {}", item.path);
    Ok(())
}

fn collect_path_leftovers(
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    path: &Path,
    source_app: &str,
) {
    if path.as_os_str().is_empty() || !path.exists() {
        return;
    }

    if validate_deletable_path(path).is_err() {
        return;
    }

    let kind = if path.is_dir() {
        JunkKind::Directory
    } else {
        JunkKind::File
    };

    let size = estimate_path_size(path);
    push_item(
        items,
        seen,
        path.display().to_string(),
        kind,
        size,
        source_app,
    );
}

fn collect_appdata_leftovers(
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    app: &ApplicationEntry,
) -> Result<()> {
    let folder_name = sanitize_folder_name(&app.display_name);
    if folder_name.is_empty() {
        return Ok(());
    }

    for base in appdata_roots() {
        let candidate = base.join(&folder_name);
        collect_path_leftovers(items, seen, &candidate, &app.display_name);
    }

    Ok(())
}

fn collect_registry_leftovers(
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    app: &ApplicationEntry,
) -> Result<()> {
    if app.source != crate::models::AppSource::Registry {
        return Ok(());
    }

    if registry_key_exists(&app.id) {
        push_item(
            items,
            seen,
            app.id.clone(),
            JunkKind::RegistryKey,
            None,
            &app.display_name,
        );
    }

    let display_lower = app.display_name.to_ascii_lowercase();
    let roots = [
        (HKEY_LOCAL_MACHINE, RegistryHive::LocalMachine, r"SOFTWARE"),
        (HKEY_CURRENT_USER, RegistryHive::CurrentUser, r"SOFTWARE"),
        (
            HKEY_LOCAL_MACHINE,
            RegistryHive::LocalMachine,
            r"SOFTWARE\WOW6432Node",
        ),
    ];

    for (hive, hive_label, base_path) in roots {
        let root = RegKey::predef(hive);
        let Ok(base) = root.open_subkey(base_path) else {
            continue;
        };
        let rooted_path = format!("{}\\{base_path}", hive_label.label());
        scan_registry_tree_by_display_name(
            &base,
            &rooted_path,
            &display_lower,
            items,
            seen,
            &app.display_name,
            0,
        )?;
    }

    Ok(())
}

fn scan_registry_tree_by_display_name(
    key: &RegKey,
    current_path: &str,
    display_lower: &str,
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    source_app: &str,
    depth: usize,
) -> Result<()> {
    if depth >= MAX_REGISTRY_SCAN_DEPTH {
        return Ok(());
    }

    for subkey_name in key.enum_keys().filter_map(Result::ok) {
        let full_path = format!(r"{current_path}\{subkey_name}");
        let subkey = match key.open_subkey(&subkey_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        if current_path.split('\\').count() >= 4 {
            if let Ok(display_name) = subkey.get_value::<String, _>("DisplayName") {
                if display_name.to_ascii_lowercase() == *display_lower
                    && is_deletable_registry_path(&full_path)
                {
                    push_item(
                        items,
                        seen,
                        full_path.clone(),
                        JunkKind::RegistryKey,
                        None,
                        source_app,
                    );
                }
            }
        }

        scan_registry_tree_by_display_name(
            &subkey,
            &full_path,
            display_lower,
            items,
            seen,
            source_app,
            depth + 1,
        )?;
    }

    Ok(())
}

fn registry_key_exists(path: &str) -> bool {
    let Ok(parsed) = parse_registry_path(path) else {
        return false;
    };

    RegKey::predef(parsed.hive.hkey())
        .open_subkey(format!("{}\\{}", parsed.parent_path, parsed.key_name))
        .is_ok()
}

fn push_item(
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    path: String,
    kind: JunkKind,
    size_estimate: Option<u64>,
    source_app: &str,
) {
    if !seen.insert(path.clone()) {
        return;
    }

    items.push(JunkItem {
        id: path.clone(),
        path,
        kind,
        size_estimate,
        source_app: Some(source_app.to_string()),
    });
}

fn estimate_path_size(path: &Path) -> Option<u64> {
    if path.is_file() {
        return path.metadata().ok().map(|m| m.len());
    }

    if !path.is_dir() {
        return None;
    }

    let mut total = 0u64;
    let entries = fs::read_dir(path).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let entry_path = entry.path();
        if entry_path.is_file() {
            total = total.saturating_add(entry.metadata().ok()?.len());
        } else if entry_path.is_dir() {
            total = total.saturating_add(estimate_path_size(&entry_path)?);
        }
    }
    Some(total)
}

pub(crate) fn delete_registry_key(path: &str) -> Result<()> {
    let parsed = parse_registry_path(path)?;
    let root = RegKey::predef(parsed.hive.hkey());
    let parent = root
        .open_subkey_with_flags(&parsed.parent_path, KEY_WRITE)
        .with_context(|| format!("failed to open registry parent for {path}"))?;
    parent
        .delete_subkey(&parsed.key_name)
        .with_context(|| format!("failed to delete registry key {path}"))?;
    Ok(())
}

fn appdata_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["LOCALAPPDATA", "APPDATA", "PROGRAMDATA"] {
        if let Ok(value) = std::env::var(var) {
            roots.push(PathBuf::from(value));
        }
    }
    roots
}

fn sanitize_folder_name(name: &str) -> String {
    name.chars()
        .filter(|c| !r#"<>:"/\|?*"#.contains(*c))
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, UninstallerType};
    use crate::registry_util::RegistryHive;

    #[test]
    fn sanitize_folder_name_strips_invalid_chars() {
        assert_eq!(sanitize_folder_name(r#"Test: App"#), "Test App");
    }

    #[test]
    fn scan_nonexistent_install_location_is_empty() {
        let app = ApplicationEntry {
            id: "test".into(),
            display_name: "Missing App".into(),
            version: None,
            publisher: None,
            source: AppSource::Registry,
            install_location: Some(r"C:\NoSuchAppFolder12345".into()),
            uninstall_string: None,
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: true,
            rating: None,
        };

        let items = scan_for_app(&app).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn delete_registry_key_parses_hklm_path() {
        let err = delete_registry_key(r"HKLM\SOFTWARE\MissingVendor\MissingApp").unwrap_err();
        assert!(err.to_string().contains("failed to open registry parent"));
    }

    #[test]
    fn delete_registry_key_parses_hkcu_path() {
        let err = delete_registry_key(r"HKCU\SOFTWARE\MissingVendor\MissingApp").unwrap_err();
        assert!(err.to_string().contains("failed to open registry parent"));
    }

    #[test]
    fn delete_registry_key_rejects_missing_hive() {
        assert!(delete_registry_key(r"SOFTWARE\Vendor\App").is_err());
    }

    #[test]
    fn delete_junk_item_rejects_vendor_registry_root() {
        let item = JunkItem {
            id: r"HKLM\SOFTWARE\Adobe".into(),
            path: r"HKLM\SOFTWARE\Adobe".into(),
            kind: JunkKind::RegistryKey,
            size_estimate: None,
            source_app: Some("Adobe".into()),
        };
        assert!(delete_junk_item(&item).is_err());
    }

    #[test]
    fn registry_key_exists_checks_parseable_paths() {
        assert!(!registry_key_exists("not-a-registry-path"));
        assert!(!registry_key_exists(
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{00000000-0000-0000-0000-000000000000}"
        ));
    }

    #[test]
    fn parse_registry_path_round_trip_hive_labels() {
        let parsed = parse_registry_path(r"HKCU\SOFTWARE\Vendor\App").unwrap();
        assert_eq!(parsed.hive, RegistryHive::CurrentUser);
    }
}
