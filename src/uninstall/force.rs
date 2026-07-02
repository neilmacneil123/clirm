use crate::models::{AppSource, ApplicationEntry};
use crate::path_safety::validate_deletable_path;
use crate::registry_util::parse_registry_path;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use winreg::enums::KEY_WRITE;
use winreg::RegKey;

/// Force-remove an application by deleting its install location and registry entries.
pub fn force_uninstall(app: &ApplicationEntry) -> Result<()> {
    if app.is_protected {
        anyhow::bail!("application is protected: {}", app.display_name);
    }

    if app.is_system_component {
        anyhow::bail!(
            "cannot force uninstall system component: {}",
            app.display_name
        );
    }

    tracing::warn!("force uninstalling {}", app.display_name);

    if let Some(location) = &app.install_location {
        let trimmed = location.trim();
        if !trimmed.is_empty() {
            delete_path(Path::new(trimmed))?;
        }
    }

    if app.source == AppSource::Registry {
        remove_registry_uninstall_key(&app.id)?;
    }

    remove_appdata_leftovers(app)?;

    Ok(())
}

fn delete_path(path: &Path) -> Result<()> {
    validate_deletable_path(path)?;

    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to delete directory {}", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("failed to delete file {}", path.display()))?;
    }

    tracing::info!("deleted {}", path.display());
    Ok(())
}

fn remove_registry_uninstall_key(app_id: &str) -> Result<()> {
    let parsed = parse_registry_path(app_id)?;
    let root = RegKey::predef(parsed.hive.hkey());
    let parent = root
        .open_subkey_with_flags(&parsed.parent_path, KEY_WRITE)
        .with_context(|| format!("failed to open registry key {app_id}"))?;

    parent
        .delete_subkey(&parsed.key_name)
        .with_context(|| format!("failed to delete registry key {app_id}"))?;

    tracing::info!("removed registry key {app_id}");
    Ok(())
}

fn remove_appdata_leftovers(app: &ApplicationEntry) -> Result<()> {
    let name = sanitize_folder_name(&app.display_name);
    if name.is_empty() {
        return Ok(());
    }

    for base in appdata_roots() {
        let candidate = base.join(&name);
        if validate_deletable_path(&candidate).is_ok() {
            delete_path(&candidate)?;
        }
    }

    Ok(())
}

fn appdata_roots() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    for var in ["LOCALAPPDATA", "APPDATA", "PROGRAMDATA"] {
        if let Ok(value) = std::env::var(var) {
            roots.push(std::path::PathBuf::from(value));
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
    use crate::registry_util::RegistryHive;

    #[test]
    fn parse_hklm_registry_id() {
        let parsed =
            parse_registry_path(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}")
                .unwrap();
        assert_eq!(parsed.hive, RegistryHive::LocalMachine);
        assert_eq!(
            parsed.parent_path,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
        );
        assert_eq!(parsed.key_name, "{GUID}");
    }

    #[test]
    fn sanitize_removes_invalid_chars() {
        assert_eq!(sanitize_folder_name(r#"Foo: Bar?"#), "Foo Bar");
    }

    #[test]
    fn delete_path_rejects_unsafe_location() {
        assert!(delete_path(Path::new(r"C:\Windows")).is_err());
        assert!(delete_path(Path::new("")).is_err());
    }
}
