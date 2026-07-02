use crate::models::{AppSource, ApplicationEntry};
use crate::uninstall::quiet;
use anyhow::{bail, Result};

/// Uninstall using quiet mode (TUI default).
pub fn uninstall_application(app: &ApplicationEntry) -> Result<()> {
    uninstall_application_with_options(app, true)
}

/// Uninstall with explicit quiet/loud control (BCU `/Q` semantics).
pub fn uninstall_application_with_options(app: &ApplicationEntry, quiet: bool) -> Result<()> {
    if app.is_protected {
        bail!("application is protected: {}", app.display_name);
    }

    if app.is_system_component && !is_windows_managed_source(app.source) {
        bail!("cannot uninstall system component: {}", app.display_name);
    }

    if quiet {
        quiet::run_quiet_uninstall(app)
    } else {
        quiet::run_loud_uninstall(app)
    }
}

fn is_windows_managed_source(source: AppSource) -> bool {
    matches!(source, AppSource::WindowsFeature | AppSource::WindowsUpdate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::UninstallerType;

    fn sample_app(source: AppSource) -> ApplicationEntry {
        ApplicationEntry {
            id: "id".into(),
            display_name: "Test".into(),
            version: None,
            publisher: None,
            source,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: true,
            is_orphan: false,
            rating: None,
        }
    }

    #[test]
    fn windows_feature_allowed_despite_system_component_flag() {
        let app = sample_app(AppSource::WindowsFeature);
        assert!(app.can_uninstall());
    }

    #[test]
    fn registry_system_component_blocked() {
        let app = sample_app(AppSource::Registry);
        assert!(!app.can_uninstall());
    }
}
