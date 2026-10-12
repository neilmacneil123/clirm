use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

pub fn scan_steam() -> Result<Vec<ApplicationEntry>> {
    let Some(steam_path) = find_steam_path() else {
        return Ok(Vec::new());
    };

    let mut libraries = vec![steam_path.clone()];
    libraries.extend(parse_library_folders(
        &steam_path.join("steamapps/libraryfolders.vdf"),
    ));

    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for library in libraries {
        let steamapps = library.join("steamapps");
        let Ok(read_dir) = fs::read_dir(&steamapps) else {
            continue;
        };

        for entry in read_dir.filter_map(Result::ok) {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            if let Some(app) = parse_app_manifest(&path) {
                if seen.insert(app.id.clone()) {
                    entries.push(app);
                }
            }
        }
    }

    Ok(entries)
}

fn find_steam_path() -> Option<PathBuf> {
    let hk = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hk
        .open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam")
        .or_else(|_| hk.open_subkey(r"SOFTWARE\Valve\Steam"))
        .ok()?;
    let install: String = key.get_value("InstallPath").ok()?;
    let path = PathBuf::from(install);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn parse_library_folders(path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"path\"") {
            if let Some(value) = extract_vdf_string(rest) {
                paths.push(PathBuf::from(value.replace("\\\\", "\\")));
            }
        }
    }
    paths
}

fn extract_vdf_string(rest: &str) -> Option<String> {
    let quote_start = rest.find('"')?;
    let after_key = &rest[quote_start + 1..];
    let quote_end = after_key.find('"')?;
    Some(after_key[..quote_end].to_string())
}

fn parse_app_manifest(path: &Path) -> Option<ApplicationEntry> {
    let contents = fs::read_to_string(path).ok()?;
    let app_id = vdf_value(&contents, "appid")?;
    let name = vdf_value(&contents, "name")?;
    let install_dir = vdf_value(&contents, "installdir")?;

    let uninstall = format!("steam://uninstall/{app_id}");
    Some(ApplicationEntry {
        id: format!("steam:{app_id}"),
        display_name: name,
        version: None,
        publisher: Some("Steam".into()),
        source: AppSource::Steam,
        install_location: Some(install_dir),
        uninstall_string: Some(uninstall.clone()),
        quiet_uninstall_string: Some(uninstall),
        uninstaller_type: UninstallerType::Unknown,
        size_estimate: None,
        install_date: None,
        is_protected: false,
        is_system_component: false,
        is_orphan: false,
        rating: None,
    })
}

fn vdf_value(contents: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            return extract_vdf_string(trimmed.trim_start_matches(&pattern));
        }
    }
    None
}
