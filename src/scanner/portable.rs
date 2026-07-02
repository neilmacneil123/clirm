use crate::config::Config;
use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const PORTABLE_MARKERS: &[&str] = &["portable", "portableapps"];

pub fn scan_portable(config: &Config) -> Result<Vec<ApplicationEntry>> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for base in &config.portable_scan_paths {
        let base_path = Path::new(base);
        if !base_path.exists() {
            continue;
        }
        scan_directory(base_path, 2, &mut entries, &mut seen)?;
    }

    Ok(entries)
}

fn scan_directory(
    dir: &Path,
    depth: u8,
    entries: &mut Vec<ApplicationEntry>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    if depth == 0 {
        return Ok(());
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };

    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if is_portable_folder(&path) {
                if let Some(app) = portable_entry(&path) {
                    if seen.insert(app.id.clone()) {
                        entries.push(app);
                    }
                }
            }
            let _ = scan_directory(&path, depth - 1, entries, seen);
        }
    }

    Ok(())
}

fn is_portable_folder(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if PORTABLE_MARKERS.iter().any(|m| name.contains(m)) {
        return true;
    }

    path.join("App").join("AppInfo.ini").exists()
        || path.join("AppInfo.ini").exists()
        || find_exe(path).is_some()
}

fn portable_entry(path: &Path) -> Option<ApplicationEntry> {
    let display_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)?;

    let install = path.to_string_lossy().to_string();
    let id = format!("portable:{install}");

    Some(ApplicationEntry {
        id,
        display_name,
        version: None,
        publisher: None,
        source: AppSource::Portable,
        install_location: Some(install.clone()),
        uninstall_string: Some(format!("rmdir /S /Q \"{install}\"")),
        quiet_uninstall_string: None,
        uninstaller_type: UninstallerType::Unknown,
        size_estimate: None,
        install_date: None,
        is_protected: false,
        is_system_component: false,
        is_orphan: true,
        rating: None,
    })
}

fn find_exe(dir: &Path) -> Option<std::path::PathBuf> {
    let read_dir = fs::read_dir(dir).ok()?;
    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            return Some(path);
        }
    }
    None
}