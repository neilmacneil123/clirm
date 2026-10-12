use anyhow::{Context, Result};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryHive {
    LocalMachine,
    CurrentUser,
}

impl RegistryHive {
    pub fn hkey(self) -> winreg::HKEY {
        match self {
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::CurrentUser => HKEY_CURRENT_USER,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LocalMachine => "HKLM",
            Self::CurrentUser => "HKCU",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "HKLM" => Some(Self::LocalMachine),
            "HKCU" => Some(Self::CurrentUser),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRegistryPath {
    pub hive: RegistryHive,
    pub parent_path: String,
    pub key_name: String,
}

/// Parse `HKLM\SOFTWARE\Vendor\App` or a registry app id into hive, parent, and leaf key.
pub fn parse_registry_path(path: &str) -> Result<ParsedRegistryPath> {
    let hive_end = path
        .find('\\')
        .context("invalid registry path: missing hive")?;
    let (hive_label, rest) = path.split_at(hive_end);
    let rest = &rest[1..];

    let key_start = rest
        .rfind('\\')
        .context("invalid registry path: missing key name")?;
    let parent_path = &rest[..key_start];
    let key_name = &rest[key_start + 1..];

    if parent_path.is_empty() || key_name.is_empty() {
        anyhow::bail!("invalid registry path: {path}");
    }

    let hive = RegistryHive::from_label(hive_label)
        .with_context(|| format!("unsupported registry hive: {hive_label}"))?;

    Ok(ParsedRegistryPath {
        hive,
        parent_path: parent_path.to_string(),
        key_name: key_name.to_string(),
    })
}

/// Registry paths eligible for deletion must live under SOFTWARE and not be vendor roots.
pub fn is_deletable_registry_path(path: &str) -> bool {
    let Ok(parsed) = parse_registry_path(path) else {
        return false;
    };

    let subpath = format!("{}\\{}", parsed.parent_path, parsed.key_name);
    let lower = subpath.to_ascii_lowercase();

    if !lower.starts_with("software\\") {
        return false;
    }

    if lower.contains(r"\uninstall\") {
        return true;
    }

    // Reject vendor roots like HKLM\SOFTWARE\Adobe (only two segments under SOFTWARE).
    subpath.split('\\').count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hklm_path() {
        let parsed = parse_registry_path(r"HKLM\SOFTWARE\Vendor\App").unwrap();
        assert_eq!(parsed.hive, RegistryHive::LocalMachine);
        assert_eq!(parsed.parent_path, "SOFTWARE\\Vendor");
        assert_eq!(parsed.key_name, "App");
    }

    #[test]
    fn parse_hkcu_path() {
        let parsed =
            parse_registry_path(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}")
                .unwrap();
        assert_eq!(parsed.hive, RegistryHive::CurrentUser);
        assert_eq!(
            parsed.parent_path,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
        );
        assert_eq!(parsed.key_name, "{GUID}");
    }

    #[test]
    fn parse_rejects_missing_hive() {
        assert!(parse_registry_path(r"SOFTWARE\Vendor").is_err());
    }

    #[test]
    fn deletable_allows_uninstall_key() {
        assert!(is_deletable_registry_path(
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}"
        ));
    }

    #[test]
    fn deletable_rejects_vendor_root() {
        assert!(!is_deletable_registry_path(r"HKLM\SOFTWARE\Adobe"));
    }

    #[test]
    fn deletable_allows_nested_app_key() {
        assert!(is_deletable_registry_path(
            r"HKLM\SOFTWARE\Adobe\Acrobat Reader"
        ));
    }
}
