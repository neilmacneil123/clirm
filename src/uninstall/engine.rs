use crate::models::ApplicationEntry;
use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn uninstall_application(app: &ApplicationEntry) -> Result<()> {
    if app.is_protected {
        bail!("application is protected: {}", app.display_name);
    }

    if app.is_system_component {
        bail!("cannot uninstall system component: {}", app.display_name);
    }

    let uninstall_cmd = app
        .quiet_uninstall_string
        .as_deref()
        .or(app.uninstall_string.as_deref())
        .with_context(|| format!("no uninstall command for {}", app.display_name))?;

    run_uninstall_command(uninstall_cmd, &app.display_name)
}

fn run_uninstall_command(command: &str, display_name: &str) -> Result<()> {
    tracing::info!("uninstalling {display_name}: {command}");

    let status = Command::new("cmd")
        .args(["/C", command])
        .status()
        .with_context(|| format!("failed to launch uninstaller for {display_name}"))?;

    if status.success() {
        Ok(())
    } else {
        bail!(
            "uninstaller exited with status {:?} for {}",
            status.code(),
            display_name
        )
    }
}
