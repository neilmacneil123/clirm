use crate::models::{ApplicationEntry, JunkItem, JunkKind};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

pub fn scan_for_app(app: &ApplicationEntry) -> Result<Vec<JunkItem>> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Some(location) = &app.install_location {
        collect_path_leftovers(
            &mut items,
            &mut seen,
            Path::new(location),
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
                Path::new(location),
                &app.display_name,
            );
        }
        collect_appdata_leftovers(&mut items, &mut seen, app)?;
    }

    Ok(items)
}

pub fn delete_junk_item(item: &JunkItem) -> Result<()> {
    match item.kind {
        JunkKind::File => {
            if Path::new(&item.path).exists() {
                fs::remove_file(&item.path)
                    .with_context(|| format!("failed to delete file {}", item.path))?;
            }
        }
        JunkKind::Directory => {
            if Path::new(&item.path).exists() {
                fs::remove_dir_all(&item.path)
                    .with_context(|| format!("failed to delete directory {}", item.path))?;
            }
        }
        JunkKind::RegistryKey => {
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
    if !path.exists() {
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

    let display_lower = app.display_name.to_lowercase();
    let publisher_lower = app.publisher.as_ref().map(|p| p.to_lowercase());

    let roots = [
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE"),
        (HKEY_CURRENT_USER, "HKCU", r"SOFTWARE"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\WOW6432Node"),
    ];

    for (hive, hive_label, base_path) in roots {
        let root = RegKey::predef(hive);
        let Ok(base) = root.open_subkey(base_path) else {
            continue;
        };
        let rooted_path = format!(r"{hive_label}\{base_path}");
        scan_registry_tree(
            &base,
            &rooted_path,
            &display_lower,
            publisher_lower.as_deref(),
            items,
            seen,
            &app.display_name,
        )?;
    }

    Ok(())
}

fn scan_registry_tree(
    key: &RegKey,
    current_path: &str,
    display_lower: &str,
    publisher_lower: Option<&str>,
    items: &mut Vec<JunkItem>,
    seen: &mut std::collections::HashSet<String>,
    source_app: &str,
) -> Result<()> {
    for subkey_name in key.enum_keys().filter_map(Result::ok) {
        let full_path = format!(r"{current_path}\{subkey_name}");
        let subkey = match key.open_subkey(&subkey_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let name_match = subkey_name.to_lowercase().contains(display_lower);
        let publisher_match = publisher_lower
            .map(|publisher| subkey_name.to_lowercase().contains(publisher))
            .unwrap_or(false);

        if name_match || publisher_match {
            push_item(
                items,
                seen,
                full_path.clone(),
                JunkKind::RegistryKey,
                None,
                source_app,
            );
        }

        if full_path.split('\\').count() < 6 {
            scan_registry_tree(
                &subkey,
                &full_path,
                display_lower,
                publisher_lower,
                items,
                seen,
                source_app,
            )?;
        }
    }

    Ok(())
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

fn delete_registry_key(path: &str) -> Result<()> {
    let (hive, subpath) = if let Some(rest) = path.strip_prefix("HKCU\\") {
        (HKEY_CURRENT_USER, rest)
    } else if let Some(rest) = path.strip_prefix("HKLM\\") {
        (HKEY_LOCAL_MACHINE, rest)
    } else {
        anyhow::bail!("invalid registry path (missing hive): {path}");
    };

    let mut segments: Vec<&str> = subpath.split('\\').collect();
    let key_name = segments.pop().context("registry path missing key name")?;
    let parent_path = segments.join("\\");

    let root = RegKey::predef(hive);
    let parent = root
        .open_subkey_with_flags(&parent_path, KEY_WRITE)
        .with_context(|| format!("failed to open registry parent for {path}"))?;
    parent
        .delete_subkey(key_name)
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
}
