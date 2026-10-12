use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::{Context, Result};
use std::collections::HashSet;
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const WOW6432_PATH: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

fn read_string(key: &RegKey, name: &str) -> Option<String> {
    key.get_value(name).ok()
}

fn read_dword(key: &RegKey, name: &str) -> Option<u32> {
    key.get_value(name).ok()
}

fn parse_entry(
    hive: &str,
    path: &str,
    subkey_name: &str,
    key: &RegKey,
) -> Option<ApplicationEntry> {
    let display_name = read_string(key, "DisplayName")?;
    if display_name.trim().is_empty() {
        return None;
    }

    let uninstall_string = read_string(key, "UninstallString");
    let quiet_uninstall_string = read_string(key, "QuietUninstallString");

    if uninstall_string.is_none() && quiet_uninstall_string.is_none() {
        return None;
    }

    let uninstaller_type = uninstall_string
        .as_deref()
        .or(quiet_uninstall_string.as_deref())
        .map(UninstallerType::detect)
        .unwrap_or(UninstallerType::Unknown);

    let system_component = read_dword(key, "SystemComponent")
        .map(|v| v != 0)
        .unwrap_or(false);

    let is_protected = read_dword(key, "NoRemove")
        .map(|v| v != 0)
        .unwrap_or(false);

    let id = format!("{hive}\\{path}\\{subkey_name}");

    Some(ApplicationEntry {
        id,
        display_name,
        version: read_string(key, "DisplayVersion"),
        publisher: read_string(key, "Publisher"),
        source: AppSource::Registry,
        install_location: read_string(key, "InstallLocation"),
        uninstall_string,
        quiet_uninstall_string,
        uninstaller_type,
        size_estimate: read_dword(key, "EstimatedSize").map(|kb| u64::from(kb) * 1024),
        install_date: read_string(key, "InstallDate"),
        is_protected,
        is_system_component: system_component,
        rating: None,
    })
}

fn scan_hive(
    root: RegKey,
    hive_label: &str,
    path: &str,
    seen: &mut HashSet<String>,
) -> Result<Vec<ApplicationEntry>> {
    let mut entries = Vec::new();
    let hive = root
        .open_subkey(path)
        .with_context(|| format!("failed to open registry path {path}"))?;

    for subkey_name in hive.enum_keys().filter_map(Result::ok) {
        let entry_id = format!("{hive_label}\\{path}\\{subkey_name}");
        if !seen.insert(entry_id) {
            continue;
        }

        let subkey = match hive.open_subkey(&subkey_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        if let Some(entry) = parse_entry(hive_label, path, &subkey_name, &subkey) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

pub fn scan_registry() -> Result<Vec<ApplicationEntry>> {
    let mut seen = HashSet::new();
    let mut all = Vec::new();

    let paths = [
        (HKEY_LOCAL_MACHINE, "HKLM", UNINSTALL_PATH),
        (HKEY_LOCAL_MACHINE, "HKLM", WOW6432_PATH),
        (HKEY_CURRENT_USER, "HKCU", UNINSTALL_PATH),
        (HKEY_CURRENT_USER, "HKCU", WOW6432_PATH),
    ];

    for (root, label, path) in paths {
        match scan_hive(RegKey::predef(root), label, path, &mut seen) {
            Ok(mut entries) => all.append(&mut entries),
            Err(e) => tracing::warn!("registry scan failed for {label}\\{path}: {e}"),
        }
    }

    Ok(all)
}