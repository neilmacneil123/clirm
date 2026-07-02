use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::{Context, Result};
use std::process::Command;

#[derive(serde::Deserialize)]
struct AppxPackage {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "PackageFullName")]
    package_full_name: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "Publisher")]
    publisher: Option<String>,
    #[serde(rename = "InstallLocation")]
    install_location: Option<String>,
}

pub fn scan_store_apps() -> Result<Vec<ApplicationEntry>> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-AppxPackage | Select-Object Name, PackageFullName, Version, Publisher, InstallLocation | ConvertTo-Json -Compress",
        ])
        .output()
        .context("failed to run Get-AppxPackage")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Get-AppxPackage failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let packages: Vec<AppxPackage> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).context("failed to parse AppxPackage array")?
    } else {
        vec![serde_json::from_str(trimmed).context("failed to parse AppxPackage object")?]
    };

    let mut entries = Vec::new();
    for pkg in packages {
        let Some(name) = pkg.name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let full_name = pkg.package_full_name.unwrap_or_else(|| name.clone());
        let uninstall = format!(
            "powershell -NoProfile -Command \"Remove-AppxPackage -Package '{}'\"",
            full_name.replace('\'', "''")
        );

        entries.push(ApplicationEntry {
            id: format!("store:{full_name}"),
            display_name: name,
            version: pkg.version,
            publisher: pkg.publisher,
            source: AppSource::Store,
            install_location: pkg.install_location,
            uninstall_string: Some(uninstall.clone()),
            quiet_uninstall_string: Some(uninstall),
            uninstaller_type: UninstallerType::StoreApp,
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
