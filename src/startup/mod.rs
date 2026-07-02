use crate::config::Config;
use crate::models::{StartupEntry, StartupLocation};
use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const RUN_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_WOW_PATH: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const RUN_DISABLED_SUFFIX: &str = "-Disabled";

fn scan_run_key(
    root: RegKey,
    hive_label: &str,
    path: &str,
    location: StartupLocation,
) -> Result<Vec<StartupEntry>> {
    let mut entries = Vec::new();
    let key = match root.open_subkey(path) {
        Ok(k) => k,
        Err(_) => return Ok(entries),
    };

    for value_name in key.enum_values().filter_map(Result::ok) {
        let (name, _) = value_name;
        let Ok(command) = key.get_value::<String, _>(&name) else {
            continue;
        };
        if command.trim().is_empty() {
            continue;
        }

        entries.push(StartupEntry {
            id: format!("reg:{hive_label}:{path}:{name}"),
            name: name.clone(),
            command,
            location,
            enabled: true,
            registry_hive: Some(hive_label.to_string()),
            registry_path: Some(path.to_string()),
            registry_value_name: Some(name),
            folder_path: None,
        });
    }

    Ok(entries)
}

fn scan_startup_folder(path: &Path, location: StartupLocation) -> Result<Vec<StartupEntry>> {
    let mut entries = Vec::new();
    if !path.is_dir() {
        return Ok(entries);
    }

    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }

        let full_path = entry.path();
        let command = full_path.to_string_lossy().into_owned();
        entries.push(StartupEntry {
            id: format!("folder:{}", full_path.display()),
            name: file_name,
            command: command.clone(),
            location,
            enabled: true,
            registry_hive: None,
            registry_path: None,
            registry_value_name: None,
            folder_path: Some(command),
        });
    }

    Ok(entries)
}

pub fn scan_startup_entries() -> Result<Vec<StartupEntry>> {
    let mut all = Vec::new();

    let run_locations = [
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            RUN_PATH,
            StartupLocation::RunKeyLocalMachine,
        ),
        (
            HKEY_LOCAL_MACHINE,
            "HKLM",
            RUN_WOW_PATH,
            StartupLocation::RunKeyLocalMachineWow,
        ),
        (
            HKEY_CURRENT_USER,
            "HKCU",
            RUN_PATH,
            StartupLocation::RunKeyCurrentUser,
        ),
    ];

    for (root, label, path, location) in run_locations {
        match scan_run_key(RegKey::predef(root), label, path, location) {
            Ok(mut entries) => all.append(&mut entries),
            Err(e) => tracing::warn!("startup scan failed for {label}\\{path}: {e}"),
        }
    }

    if let Ok(appdata) = env::var("APPDATA") {
        let user_startup =
            PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        match scan_startup_folder(&user_startup, StartupLocation::StartupFolderUser) {
            Ok(mut entries) => all.append(&mut entries),
            Err(e) => tracing::warn!("user startup folder scan failed: {e}"),
        }
    }

    let all_users_startup =
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp");
    match scan_startup_folder(&all_users_startup, StartupLocation::StartupFolderAllUsers) {
        Ok(mut entries) => all.append(&mut entries),
        Err(e) => tracing::warn!("all-users startup folder scan failed: {e}"),
    }

    all.sort_by_key(|a| a.name.to_lowercase());
    Ok(all)
}

pub fn disable_startup_entry(entry: &StartupEntry) -> Result<()> {
    if entry.location.is_registry() {
        disable_registry_run_value(entry)
    } else {
        disable_startup_folder_item(entry)
    }
}

fn disabled_run_path(path: &str) -> String {
    if path.ends_with(RUN_DISABLED_SUFFIX) {
        path.to_string()
    } else {
        format!("{path}{RUN_DISABLED_SUFFIX}")
    }
}

fn disable_registry_run_value(entry: &StartupEntry) -> Result<()> {
    let hive_label = entry
        .registry_hive
        .as_deref()
        .context("missing registry hive on startup entry")?;
    let path = entry
        .registry_path
        .as_deref()
        .context("missing registry path on startup entry")?;
    let value_name = entry
        .registry_value_name
        .as_deref()
        .context("missing registry value name on startup entry")?;

    let hive = match hive_label {
        "HKLM" => HKEY_LOCAL_MACHINE,
        "HKCU" => HKEY_CURRENT_USER,
        other => bail!("unsupported hive: {other}"),
    };

    let run_key = RegKey::predef(hive)
        .open_subkey_with_flags(path, KEY_READ | KEY_SET_VALUE)
        .with_context(|| format!("failed to open registry key {path}"))?;
    let command: String = run_key
        .get_value(value_name)
        .with_context(|| format!("startup value {value_name} not found"))?;

    let backup_path = disabled_run_path(path);
    let backup_key = RegKey::predef(hive)
        .create_subkey(&backup_path)
        .with_context(|| format!("failed to create backup key {backup_path}"))?
        .0;
    backup_key
        .set_value(value_name, &command)
        .with_context(|| format!("failed to back up startup value {value_name}"))?;

    run_key
        .delete_value(value_name)
        .with_context(|| format!("failed to remove startup value {value_name}"))?;
    Ok(())
}

fn startup_backup_dir() -> Result<PathBuf> {
    let dir = Config::config_dir()?.join("startup-backup");
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create backup directory {}", dir.display()))?;
    Ok(dir)
}

fn disable_startup_folder_item(entry: &StartupEntry) -> Result<()> {
    let source = entry
        .folder_path
        .as_deref()
        .or(Some(entry.command.as_str()))
        .context("missing folder path on startup entry")?;
    let path = Path::new(source);
    if !path.exists() {
        bail!("startup file not found: {}", path.display());
    }

    let backup_dir = startup_backup_dir()?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("startup-item");
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_path = backup_dir.join(format!("{file_name}-{stamp}"));

    fs::rename(path, &backup_path).with_context(|| {
        format!(
            "failed to move {} to backup {}",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_returns_sorted_entries() {
        let entries = scan_startup_entries().unwrap();
        for window in entries.windows(2) {
            assert!(window[0].name.to_lowercase() <= window[1].name.to_lowercase());
        }
    }

    #[test]
    fn disabled_run_path_appends_suffix() {
        assert_eq!(
            disabled_run_path(RUN_PATH),
            format!("{RUN_PATH}{RUN_DISABLED_SUFFIX}")
        );
    }
}
