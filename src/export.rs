use crate::models::ApplicationEntry;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_csv(apps: &[ApplicationEntry], path: &Path) -> Result<()> {
    let mut file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    writeln!(
        file,
        "id,display_name,version,publisher,source,install_location,size_bytes,is_protected"
    )?;
    for app in apps {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{}",
            csv_escape(&app.id),
            csv_escape(&app.display_name),
            csv_escape(app.version.as_deref().unwrap_or("")),
            csv_escape(app.publisher.as_deref().unwrap_or("")),
            app.source.label(),
            csv_escape(app.install_location.as_deref().unwrap_or("")),
            app.size_estimate.unwrap_or(0),
            app.is_protected,
        )?;
    }
    Ok(())
}

pub fn export_json(apps: &[ApplicationEntry], path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(apps).context("failed to serialize apps to JSON")?;
    std::fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn default_export_path(ext: &str) -> Result<std::path::PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    Ok(crate::config::Config::config_dir()?.join(format!("clirm-export-{stamp}.{ext}")))
}
