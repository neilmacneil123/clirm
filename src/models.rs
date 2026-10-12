use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppSource {
    Registry,
}

impl AppSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Registry => "Registry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UninstallerType {
    Msi,
    InnoSetup,
    Nsis,
    Unknown,
}

impl UninstallerType {
    pub fn detect(uninstall_string: &str) -> Self {
        let lower = uninstall_string.to_lowercase();
        if lower.contains("msiexec") {
            Self::Msi
        } else if lower.contains("inno") {
            Self::InnoSetup
        } else if lower.contains("nsis") || lower.contains("uninst.exe") {
            Self::Nsis
        } else {
            Self::Unknown
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Msi => "MSI",
            Self::InnoSetup => "Inno Setup",
            Self::Nsis => "NSIS",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationEntry {
    pub id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub source: AppSource,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub quiet_uninstall_string: Option<String>,
    pub uninstaller_type: UninstallerType,
    pub size_estimate: Option<u64>,
    pub install_date: Option<String>,
    pub is_protected: bool,
    pub is_system_component: bool,
    pub rating: Option<u8>,
}

impl ApplicationEntry {
    pub fn can_uninstall(&self) -> bool {
        !self.is_protected
            && !self.is_system_component
            && (self.uninstall_string.is_some() || self.quiet_uninstall_string.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::UninstallerType;

    #[test]
    fn detect_msi_uninstaller() {
        assert_eq!(
            UninstallerType::detect(r#"C:\Windows\System32\msiexec.exe /X{GUID}"#),
            UninstallerType::Msi
        );
    }

    #[test]
    fn detect_inno_uninstaller() {
        assert_eq!(
            UninstallerType::detect(r#"C:\App\unins000.exe /Inno"#),
            UninstallerType::InnoSetup
        );
    }

    #[test]
    fn detect_nsis_uninstaller() {
        assert_eq!(
            UninstallerType::detect(r#"C:\App\uninst.exe"#),
            UninstallerType::Nsis
        );
    }

    #[test]
    fn detect_unknown_uninstaller() {
        assert_eq!(
            UninstallerType::detect(r#"C:\App\custom-remove.exe"#),
            UninstallerType::Unknown
        );
    }
}