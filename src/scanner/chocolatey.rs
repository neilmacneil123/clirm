use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::Result;
use std::process::Command;

pub fn scan_chocolatey() -> Result<Vec<ApplicationEntry>> {
    let output = Command::new("choco")
        .args(["list", "-lo", "--limit-output", "--no-color"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return Ok(Vec::new()),
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, version) = match line.split_once('|') {
            Some((n, v)) => (n.trim(), Some(v.trim().to_string())),
            None => (line, None),
        };
        if name.is_empty() {
            continue;
        }

        let uninstall = format!("choco uninstall {name} -y");
        entries.push(ApplicationEntry {
            id: format!("choco:{name}"),
            display_name: name.to_string(),
            version,
            publisher: Some("Chocolatey".into()),
            source: AppSource::Chocolatey,
            install_location: None,
            uninstall_string: Some(uninstall.clone()),
            quiet_uninstall_string: Some(uninstall),
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        });
    }

    Ok(entries)
}
