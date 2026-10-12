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

pub fn export_xml(apps: &[ApplicationEntry], path: &Path) -> Result<()> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str("<ArrayOfApplicationEntry>\n");
    for app in apps {
        out.push_str("  <ApplicationEntry>\n");
        xml_element(&mut out, "Id", &app.id);
        xml_element(&mut out, "DisplayName", &app.display_name);
        xml_element_opt(&mut out, "DisplayVersion", app.version.as_deref());
        xml_element_opt(&mut out, "Publisher", app.publisher.as_deref());
        xml_element(&mut out, "AboutUrl", app.source.label());
        xml_element_opt(&mut out, "InstallLocation", app.install_location.as_deref());
        xml_element_opt(&mut out, "UninstallString", app.uninstall_string.as_deref());
        xml_element_opt(
            &mut out,
            "QuietUninstallString",
            app.quiet_uninstall_string.as_deref(),
        );
        if let Some(size) = app.size_estimate {
            xml_element(&mut out, "EstimatedSize", &size.to_string());
        }
        xml_element_opt(&mut out, "InstallDate", app.install_date.as_deref());
        xml_element(&mut out, "IsProtected", &app.is_protected.to_string());
        xml_element(
            &mut out,
            "IsSystemComponent",
            &app.is_system_component.to_string(),
        );
        if let Some(rating) = app.rating {
            xml_element(&mut out, "Rating", &rating.to_string());
        }
        out.push_str("  </ApplicationEntry>\n");
    }
    out.push_str("</ArrayOfApplicationEntry>\n");
    std::fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn xml_element(out: &mut String, tag: &str, value: &str) {
    out.push_str("    <");
    out.push_str(tag);
    out.push('>');
    out.push_str(&xml_escape(value));
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

fn xml_element_opt(out: &mut String, tag: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        xml_element(out, tag, value);
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, UninstallerType};

    fn sample_app() -> ApplicationEntry {
        ApplicationEntry {
            id: "test-id".into(),
            display_name: "Test App".into(),
            version: Some("1.0".into()),
            publisher: Some("Publisher".into()),
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        }
    }

    #[test]
    fn xml_export_contains_application_entry() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("clirm-test-export-{}.xml", std::process::id()));
        export_xml(&[sample_app()], &path).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<ArrayOfApplicationEntry>"));
        assert!(contents.contains("<DisplayName>Test App</DisplayName>"));
        let _ = std::fs::remove_file(path);
    }
}
