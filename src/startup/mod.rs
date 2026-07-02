use crate::models::{StartupEntry, StartupLocation};
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const RUN_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_WOW_PATH: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";

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
            id: format!("{hive_label}|{path}|{name}"),
            name: name.clone(),
            command,
            location,
            enabled: true,
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
            id: format!("folder:{}\\{file_name}", path.display()),
            name: file_name,
            command,
            location,
            enabled: true,
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

fn disable_registry_run_value(entry: &StartupEntry) -> Result<()> {
    let parts: Vec<&str> = entry.id.split('|').collect();
    if parts.len() != 3 {
        anyhow::bail!("invalid startup entry id: {}", entry.id);
    }

    let hive = match parts[0] {
        "HKLM" => HKEY_LOCAL_MACHINE,
        "HKCU" => HKEY_CURRENT_USER,
        other => anyhow::bail!("unsupported hive: {other}"),
    };
    let path = parts[1];
    let value_name = parts[2];

    let key = RegKey::predef(hive)
        .open_subkey_with_flags(path, KEY_SET_VALUE)
        .with_context(|| format!("failed to open registry key {path}"))?;
    key.delete_value(value_name)
        .with_context(|| format!("failed to delete run value {value_name}"))?;
    Ok(())
}

fn disable_startup_folder_item(entry: &StartupEntry) -> Result<()> {
    let path = Path::new(&entry.command);
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
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
}
