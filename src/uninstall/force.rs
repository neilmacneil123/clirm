use crate::models::{AppSource, ApplicationEntry};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WRITE};
use winreg::RegKey;

enum RegistryHive {
    LocalMachine,
    CurrentUser,
}

impl RegistryHive {
    fn hkey(self) -> winreg::HKEY {
        match self {
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::CurrentUser => HKEY_CURRENT_USER,
        }
    }
}

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
        delete_path(Path::new(location))?;
    }

    if app.source == AppSource::Registry {
        remove_registry_uninstall_key(&app.id)?;
    }

    remove_appdata_leftovers(app)?;

    Ok(())
}

fn delete_path(path: &Path) -> Result<()> {
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
    let (hive, subpath, key_name) = parse_registry_id(app_id)?;
    let root = RegKey::predef(hive.hkey());
    let parent = root
        .open_subkey_with_flags(&subpath, KEY_WRITE)
        .with_context(|| format!("failed to open registry key {app_id}"))?;

    parent
        .delete_subkey(&key_name)
        .with_context(|| format!("failed to delete registry key {app_id}"))?;

    tracing::info!("removed registry key {app_id}");
    Ok(())
}

fn parse_registry_id(app_id: &str) -> Result<(RegistryHive, String, String)> {
    let hive_end = app_id
        .find('\\')
        .context("invalid registry app id: missing hive")?;
    let (hive_label, rest) = app_id.split_at(hive_end);
    let rest = &rest[1..];

    let key_start = rest
        .rfind('\\')
        .context("invalid registry app id: missing key name")?;
    let subpath = &rest[..key_start];
    let key_name = &rest[key_start + 1..];

    if subpath.is_empty() || key_name.is_empty() {
        anyhow::bail!("invalid registry app id: {app_id}");
    }

    let hive = match hive_label {
        "HKLM" => RegistryHive::LocalMachine,
        "HKCU" => RegistryHive::CurrentUser,
        _ => anyhow::bail!("unsupported registry hive: {hive_label}"),
    };

    Ok((hive, subpath.to_string(), key_name.to_string()))
}

fn remove_appdata_leftovers(app: &ApplicationEntry) -> Result<()> {
    let name = sanitize_folder_name(&app.display_name);
    if name.is_empty() {
        return Ok(());
    }

    for base in appdata_roots() {
        let candidate = base.join(&name);
        delete_path(&candidate)?;
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

    #[test]
    fn parse_hklm_registry_id() {
        let (hive, path, key) =
            parse_registry_id(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}")
                .unwrap();
        assert!(matches!(hive, RegistryHive::LocalMachine));
        assert_eq!(path, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall");
        assert_eq!(key, "{GUID}");
    }

    #[test]
    fn sanitize_removes_invalid_chars() {
        assert_eq!(sanitize_folder_name(r#"Foo: Bar?"#), "Foo Bar");
    }
}
