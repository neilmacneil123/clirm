use crate::models::ApplicationEntry;
use crate::uninstall::quiet;
use anyhow::bail;
use anyhow::Result;

pub fn uninstall_application(app: &ApplicationEntry) -> Result<()> {
    if app.is_protected {
        bail!("application is protected: {}", app.display_name);
    }

    if app.is_system_component {
        bail!("cannot uninstall system component: {}", app.display_name);
    }

    quiet::run_quiet_uninstall(app)
}
