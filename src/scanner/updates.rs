use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::{Context, Result};
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const WOW6432_PATH: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

fn read_string(key: &RegKey, name: &str) -> Option<String> {
    key.get_value(name).ok()
}

fn read_dword(key: &RegKey, name: &str) -> Option<u32> {
    key.get_value(name).ok()
}

fn is_update_entry(display_name: &str, parent_key: Option<&str>) -> bool {
    if parent_key.is_some() {
        return true;
    }

    let lower = display_name.to_lowercase();
    lower.contains("update for")
        || lower.starts_with("security update")
        || lower.starts_with("hotfix")
        || lower.contains("kb")
}

fn scan_hive(root: RegKey, hive_label: &str, path: &str) -> Result<Vec<ApplicationEntry>> {
    let mut entries = Vec::new();
    let hive = match root.open_subkey(path) {
        Ok(h) => h,
        Err(_) => return Ok(entries),
    };

    for subkey_name in hive.enum_keys().filter_map(Result::ok) {
        let subkey = match hive.open_subkey(&subkey_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let display_name = match read_string(&subkey, "DisplayName") {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };

        let parent_key = read_string(&subkey, "ParentKeyName");
        if !is_update_entry(&display_name, parent_key.as_deref()) {
            continue;
        }

        let uninstall_string = read_string(&subkey, "UninstallString");
        let quiet_uninstall_string = read_string(&subkey, "QuietUninstallString");
        if uninstall_string.is_none() && quiet_uninstall_string.is_none() {
            continue;
        }

        let uninstaller_type = uninstall_string
            .as_deref()
            .or(quiet_uninstall_string.as_deref())
            .map(UninstallerType::detect)
            .unwrap_or(UninstallerType::Unknown);

        let id = format!("{hive_label}\\{path}\\{subkey_name}");
        entries.push(ApplicationEntry {
            id,
            display_name,
            version: read_string(&subkey, "DisplayVersion"),
            publisher: read_string(&subkey, "Publisher").or(Some("Microsoft Corporation".into())),
            source: AppSource::WindowsUpdate,
            install_location: read_string(&subkey, "InstallLocation"),
            uninstall_string,
            quiet_uninstall_string,
            uninstaller_type,
            size_estimate: read_dword(&subkey, "EstimatedSize").map(|kb| u64::from(kb) * 1024),
            install_date: read_string(&subkey, "InstallDate"),
            is_protected: read_dword(&subkey, "NoRemove")
                .map(|v| v != 0)
                .unwrap_or(false),
            is_system_component: true,
            is_orphan: false,
            rating: None,
        });
    }

    Ok(entries)
}

fn scan_hotfixes() -> Result<Vec<ApplicationEntry>> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-HotFix | Select-Object HotFixID, Description, InstalledOn | ConvertTo-Json -Compress",
        ])
        .output()
        .context("failed to run Get-HotFix")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    #[derive(serde::Deserialize)]
    struct HotFix {
        #[serde(rename = "HotFixID")]
        hotfix_id: Option<String>,
        #[serde(rename = "Description")]
        description: Option<String>,
        #[serde(rename = "InstalledOn")]
        installed_on: Option<String>,
    }

    let hotfixes: Vec<HotFix> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).unwrap_or_default()
    } else {
        serde_json::from_str(trimmed)
            .map(|h: HotFix| vec![h])
            .unwrap_or_default()
    };

    let mut entries = Vec::new();
    for hf in hotfixes {
        let Some(hotfix_id) = hf.hotfix_id.filter(|id| !id.is_empty()) else {
            continue;
        };
        let display_name = hf
            .description
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| format!("Windows Update {hotfix_id}"));

        let kb = hotfix_id.trim_start_matches("KB");
        let uninstall = format!("wusa /uninstall /kb:{kb} /quiet /norestart");

        entries.push(ApplicationEntry {
            id: format!("hotfix:{hotfix_id}"),
            display_name,
            version: Some(hotfix_id.clone()),
            publisher: Some("Microsoft Corporation".into()),
            source: AppSource::WindowsUpdate,
            install_location: None,
            uninstall_string: Some(uninstall.clone()),
            quiet_uninstall_string: Some(uninstall),
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: hf.installed_on,
            is_protected: false,
            is_system_component: true,
            is_orphan: false,
            rating: None,
        });
    }

    Ok(entries)
}

pub fn scan_windows_updates() -> Result<Vec<ApplicationEntry>> {
    let mut all = Vec::new();
    let paths = [
        (HKEY_LOCAL_MACHINE, "HKLM", UNINSTALL_PATH),
        (HKEY_LOCAL_MACHINE, "HKLM", WOW6432_PATH),
    ];

    for (root, label, path) in paths {
        match scan_hive(RegKey::predef(root), label, path) {
            Ok(mut entries) => all.append(&mut entries),
            Err(e) => tracing::warn!("update registry scan failed for {label}\\{path}: {e}"),
        }
    }

    match scan_hotfixes() {
        Ok(mut hotfixes) => {
            let existing_ids: std::collections::HashSet<String> =
                all.iter().filter_map(|a| a.version.clone()).collect();
            for hf in hotfixes.drain(..) {
                if let Some(ver) = &hf.version {
                    if existing_ids.contains(ver) {
                        continue;
                    }
                }
                all.push(hf);
            }
        }
        Err(e) => tracing::warn!("hotfix scan failed: {e}"),
    }

    Ok(all)
}
