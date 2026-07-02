use crate::models::{ApplicationEntry, UninstallerType};
use anyhow::{bail, Context, Result};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

/// Build the best available quiet uninstall command for an application.
pub fn build_quiet_command(app: &ApplicationEntry) -> Result<String> {
    if let Some(quiet) = app.quiet_uninstall_string.as_deref() {
        return Ok(quiet.to_string());
    }

    let uninstall = app
        .uninstall_string
        .as_deref()
        .with_context(|| format!("no uninstall command for {}", app.display_name))?;

    Ok(apply_quiet_flags(uninstall, app.uninstaller_type))
}

/// Run the raw uninstall string without injecting quiet flags (BCU loud mode).
pub fn run_loud_uninstall(app: &ApplicationEntry) -> Result<()> {
    let command = app
        .uninstall_string
        .as_deref()
        .or(app.quiet_uninstall_string.as_deref())
        .with_context(|| format!("no uninstall command for {}", app.display_name))?;

    tracing::info!("loud uninstall {}: {}", app.display_name, command);
    run_command(command)
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
            run_with_gui_automation(loud, &app.display_name).with_context(|| {
                format!(
                    "quiet uninstall failed and GUI automation did not succeed for {}",
                    app.display_name
                )
            })
        }
        Err(e) => Err(e),
    }
}

/// Best-effort GUI automation is only attempted for loud/unknown uninstallers.
pub(crate) fn needs_gui_automation(app: &ApplicationEntry) -> bool {
    app.uninstaller_type == UninstallerType::Unknown
        || app.quiet_uninstall_string.is_none() && !has_quiet_flags(app)
}

pub(crate) fn has_quiet_flags(app: &ApplicationEntry) -> bool {
    let cmd = app.uninstall_string.as_deref().unwrap_or("");
    matches!(
        app.uninstaller_type,
        UninstallerType::Msi
            | UninstallerType::InnoSetup
            | UninstallerType::Nsis
            | UninstallerType::StoreApp
    ) || contains_ascii_ci(cmd, "/s")
        || contains_ascii_ci(cmd, "/silent")
        || contains_ascii_ci(cmd, "/quiet")
        || contains_ascii_ci(cmd, "/qn")
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
    if contains_ascii_ci(command, "/qn") || contains_ascii_ci(command, "/quiet") {
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
    if contains_ascii_ci(command, "/silent") || contains_ascii_ci(command, "/verysilent") {
        return command.to_string();
    }
    format!("{command} /VERYSILENT /NORESTART")
}

fn apply_nsis_quiet(command: &str) -> String {
    if contains_ascii_ci(command, " /s") || command.ends_with("/S") || command.ends_with("/s") {
        return command.to_string();
    }
    format!("{command} /S")
}

/// Launch a loud uninstaller and attempt to dismiss common confirmation dialogs.
/// Automation is best-effort and intended for `Unknown` uninstaller types only.
pub fn run_with_gui_automation(command: &str, display_name: &str) -> Result<()> {
    tracing::info!("GUI automation uninstall {display_name}: {command}");

    let mut child = Command::new("cmd")
        .args(["/C", "start", "", "/wait", command])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to launch uninstaller for {display_name}"))?;

    let automation_name = display_name.to_string();
    let automation_command = command.to_string();
    let automation_handle = thread::spawn(move || {
        if let Err(e) = dismiss_common_dialogs(&automation_name, &automation_command) {
            tracing::warn!("GUI automation helper failed for {automation_name}: {e}");
        }
    });

    let status = wait_for_child(&mut child, display_name)?;
    let _ = automation_handle;

    if status.success() {
        Ok(())
    } else {
        bail!(
            "GUI uninstaller exited with status {:?} for {}",
            status.code(),
            display_name
        );
    }
}

fn wait_for_child(child: &mut Child, display_name: &str) -> Result<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("failed waiting for uninstaller {display_name}"))
            }
        }
    }
}

fn dismiss_common_dialogs(display_name: &str, command: &str) -> Result<()> {
    let escaped_name = escape_powershell_single_quoted(display_name);
    let exe_hint = extract_exe_hint(command)
        .map(|hint| escape_powershell_single_quoted(&hint))
        .unwrap_or_default();

    let script = format!(
        r#"
$wshell = New-Object -ComObject WScript.Shell
$titles = @('{escaped_name}', '{exe_hint}') | Where-Object {{ $_ }}
$deadline = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $deadline) {{
    foreach ($title in $titles) {{
        if ($wshell.AppActivate($title)) {{
            $wshell.SendKeys('{{ENTER}}')
            Start-Sleep -Milliseconds 300
        }}
    }}
    Start-Sleep -Milliseconds 500
}}
"#
    );

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .context("failed to launch GUI automation helper")?;

    if status.success() {
        Ok(())
    } else {
        bail!(
            "GUI automation helper exited with status {:?}",
            status.code()
        );
    }
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

fn extract_exe_hint(command: &str) -> Option<String> {
    let token = command.split_whitespace().next()?;
    let path = std::path::Path::new(token);
    let file_name = path.file_name()?.to_string_lossy();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_name.to_string());
    if stem.is_empty() {
        None
    } else {
        Some(stem)
    }
}

fn contains_ascii_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }

    haystack.as_bytes().windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
    })
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

    fn sample_app(uninstaller_type: UninstallerType) -> ApplicationEntry {
        ApplicationEntry {
            id: "test".into(),
            display_name: "Test".into(),
            version: None,
            publisher: None,
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        }
    }

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

    #[test]
    fn build_quiet_command_fails_without_uninstall_string() {
        let mut app = sample_app(UninstallerType::Unknown);
        app.uninstall_string = None;
        assert!(build_quiet_command(&app).is_err());
    }

    #[test]
    fn needs_gui_automation_for_unknown_type() {
        let app = sample_app(UninstallerType::Unknown);
        assert!(needs_gui_automation(&app));
    }

    #[test]
    fn needs_gui_automation_false_for_msi_with_flags() {
        let mut app = sample_app(UninstallerType::Msi);
        app.uninstall_string = Some(r#"msiexec /X {GUID}"#.into());
        assert!(!needs_gui_automation(&app));
    }

    #[test]
    fn has_quiet_flags_detects_silent_suffix() {
        let mut app = sample_app(UninstallerType::InnoSetup);
        app.uninstall_string = Some(r#"C:\uninst.exe /SILENT"#.into());
        assert!(has_quiet_flags(&app));
        assert!(!needs_gui_automation(&app));
    }

    #[test]
    fn escape_powershell_single_quoted_doubles_apostrophes() {
        assert_eq!(escape_powershell_single_quoted("O'Brien"), "O''Brien");
    }

    #[test]
    fn extract_exe_hint_from_quoted_path() {
        assert_eq!(
            extract_exe_hint(r#""C:\App\unins000.exe" /SILENT"#),
            Some("unins000".to_string())
        );
    }

    #[test]
    fn run_quiet_uninstall_fails_for_missing_command() {
        let mut app = sample_app(UninstallerType::Unknown);
        app.uninstall_string = None;
        assert!(run_quiet_uninstall(&app).is_err());
    }

    #[test]
    fn run_quiet_uninstall_does_not_fallback_for_msi_failure() {
        let mut app = sample_app(UninstallerType::Msi);
        app.uninstall_string = Some("cmd /C exit 1".into());
        let err = run_quiet_uninstall(&app).unwrap_err();
        assert!(!err.to_string().contains("GUI automation"));
    }
}
