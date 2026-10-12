use crate::models::{AppSource, ApplicationEntry, UninstallerType};
use anyhow::{Context, Result};
use std::process::Command;

#[derive(serde::Deserialize)]
struct OptionalFeature {
    #[serde(rename = "FeatureName")]
    feature_name: Option<String>,
    #[serde(rename = "DisplayName")]
    display_name: Option<String>,
    #[serde(rename = "State")]
    _state: Option<String>,
}

pub fn scan_windows_features() -> Result<Vec<ApplicationEntry>> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-WindowsOptionalFeature -Online | Where-Object { $_.State -eq 'Enabled' } | Select-Object FeatureName, DisplayName, State | ConvertTo-Json -Compress",
        ])
        .output()
        .context("failed to run Get-WindowsOptionalFeature")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("Get-WindowsOptionalFeature failed: {stderr}");
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let features: Vec<OptionalFeature> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).context("failed to parse optional feature array")?
    } else {
        vec![serde_json::from_str(trimmed).context("failed to parse optional feature object")?]
    };

    let mut entries = Vec::new();
    for feature in features {
        let Some(feature_name) = feature.feature_name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let display_name = feature
            .display_name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| feature_name.clone());

        let uninstall =
            format!("dism /online /Disable-Feature /FeatureName:{feature_name} /NoRestart");
        let quiet_uninstall =
            format!("dism /online /Disable-Feature /FeatureName:{feature_name} /Quiet /NoRestart");

        entries.push(ApplicationEntry {
            id: format!("feature:{feature_name}"),
            display_name,
            version: None,
            publisher: Some("Microsoft Corporation".into()),
            source: AppSource::WindowsFeature,
            install_location: None,
            uninstall_string: Some(uninstall),
            quiet_uninstall_string: Some(quiet_uninstall),
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
