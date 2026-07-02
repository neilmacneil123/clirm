use crate::models::{ApplicationEntry, UninstallerType};
use anyhow::{bail, Context, Result};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Build the best available quiet uninstall command for an application.
pub fn build_quiet_command(app: &ApplicationEntry) -> Result<String> {
    if let Some(ref quiet) = app.quiet_uninstall_string {
        return Ok(quiet.clone());
    }

    let uninstall = app
        .uninstall_string
        .as_deref()
        .with_context(|| format!("no uninstall command for {}", app.display_name))?;

    Ok(apply_quiet_flags(uninstall, app.uninstaller_type))
}

/// Run an uninstall command, falling back to basic GUI automation for loud uninstallers.
pub fn run_quiet_uninstall(app: &ApplicationEntry) -> Result<()> {
    let command = build_quiet_command(app)?;
    tracing::info!("quiet uninstall {}: {}", app.display_name, command);

    match run_command(&command) {
        Ok(()) => Ok(()),
        Err(e) if needs_gui_automation(app) => {
            tracing::warn!(
                "quiet uninstall failed for {}, trying GUI automation: {e}",
                app.display_name
            );
            let loud = app.uninstall_string.as_deref().unwrap_or(&command);
            run_with_gui_automation(loud, &app.display_name)
        }
        Err(e) => Err(e),
    }
}

fn needs_gui_automation(app: &ApplicationEntry) -> bool {
    app.uninstaller_type == UninstallerType::Unknown
        || app.quiet_uninstall_string.is_none() && !has_quiet_flags(app)
}

fn has_quiet_flags(app: &ApplicationEntry) -> bool {
    let cmd = app.uninstall_string.as_deref().unwrap_or("");
    let lower = cmd.to_lowercase();
    matches!(
        app.uninstaller_type,
        UninstallerType::Msi
            | UninstallerType::InnoSetup
            | UninstallerType::Nsis
            | UninstallerType::StoreApp
    ) || lower.contains("/s")
        || lower.contains("/silent")
        || lower.contains("/quiet")
        || lower.contains("/qn")
}

pub fn apply_quiet_flags(command: &str, uninstaller_type: UninstallerType) -> String {
    match uninstaller_type {
        UninstallerType::Msi => apply_msi_quiet(command),
        UninstallerType::InnoSetup => apply_inno_quiet(command),
        UninstallerType::Nsis => apply_nsis_quiet(command),
        UninstallerType::StoreApp => command.to_string(),
        UninstallerType::Unknown => command.to_string(),
    }
}

fn apply_msi_quiet(command: &str) -> String {
    let lower = command.to_lowercase();
    if lower.contains("/qn") || lower.contains("/quiet") {
        return command.to_string();
    }

    let mut result = command.to_string();
    for flag in ["/I", "/i", "/X", "/x"] {
        if let Some(pos) = result.find(flag) {
            let end = pos + flag.len();
            result.replace_range(pos..end, "/X");
            break;
        }
    }
    format!("{result} /qn /norestart")
}

fn apply_inno_quiet(command: &str) -> String {
    let lower = command.to_lowercase();
    if lower.contains("/silent") || lower.contains("/verysilent") {
        return command.to_string();
    }
    format!("{command} /VERYSILENT /NORESTART")
}

fn apply_nsis_quiet(command: &str) -> String {
    let lower = command.to_lowercase();
    if lower.contains(" /s") || lower.ends_with("/s") {
        return command.to_string();
    }
    format!("{command} /S")
}

/// Launch a loud uninstaller and attempt to dismiss common confirmation dialogs.
pub fn run_with_gui_automation(command: &str, display_name: &str) -> Result<()> {
    tracing::info!("GUI automation uninstall {display_name}: {command}");

    let child = Command::new("cmd")
        .args(["/C", "start", "", "/wait", command])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to launch uninstaller for {display_name}"))?;

    thread::sleep(Duration::from_secs(2));
    dismiss_common_dialogs(display_name);

    let output = child
        .wait_with_output()
        .with_context(|| format!("failed waiting for uninstaller {display_name}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!(
                "GUI uninstaller exited with status {:?} for {}",
                output.status.code(),
                display_name
            );
        } else {
            bail!(
                "GUI uninstaller exited with status {:?} for {}: {}",
                output.status.code(),
                display_name,
                stderr
            );
        }
    }
}

fn dismiss_common_dialogs(display_name: &str) {
    let script = format!(
        r#"
$wshell = New-Object -ComObject WScript.Shell
$title = '{display_name}'
for ($i = 0; $i -lt 8; $i++) {{
    if ($wshell.AppActivate($title)) {{
        $wshell.SendKeys('{{ENTER}}')
        Start-Sleep -Milliseconds 500
    }}
    $wshell.SendKeys('{{ENTER}}')
    $wshell.SendKeys('{{TAB}}')
    Start-Sleep -Milliseconds 500
}}
"#
    );

    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn run_command(command: &str) -> Result<()> {
    let output = Command::new("cmd")
        .args(["/C", command])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to launch uninstall command")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("uninstaller exited with status {:?}", output.status.code());
        } else {
            bail!(
                "uninstaller exited with status {:?}: {}",
                output.status.code(),
                stderr
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppSource;

    #[test]
    fn msi_adds_quiet_flags() {
        let cmd = apply_quiet_flags(r#"msiexec.exe /X{ABC-123}"#, UninstallerType::Msi);
        assert!(cmd.contains("/qn"));
        assert!(cmd.contains("/norestart"));
    }

    #[test]
    fn inno_adds_silent_flag() {
        let cmd = apply_quiet_flags(r#"C:\App\unins000.exe"#, UninstallerType::InnoSetup);
        assert!(cmd.to_uppercase().contains("/VERYSILENT"));
    }

    #[test]
    fn nsis_adds_silent_flag() {
        let cmd = apply_quiet_flags(r#"C:\App\uninst.exe"#, UninstallerType::Nsis);
        assert!(cmd.ends_with("/S") || cmd.contains(" /S"));
    }

    #[test]
    fn prefers_registry_quiet_string() {
        let app = ApplicationEntry {
            id: "test".into(),
            display_name: "Test".into(),
            version: None,
            publisher: None,
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some(r#"C:\App\uninst.exe"#.into()),
            quiet_uninstall_string: Some(r#"C:\App\uninst.exe /SILENT"#.into()),
            uninstaller_type: UninstallerType::InnoSetup,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        };
        let cmd = build_quiet_command(&app).unwrap();
        assert_eq!(cmd, r#"C:\App\uninst.exe /SILENT"#);
    }
}
