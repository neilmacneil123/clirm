use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppSource {
    Registry,
    Store,
    Portable,
    Steam,
    Chocolatey,
    WindowsFeature,
    WindowsUpdate,
}

impl AppSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Registry => "Registry",
            Self::Store => "Store",
            Self::Portable => "Portable",
            Self::Steam => "Steam",
            Self::Chocolatey => "Chocolatey",
            Self::WindowsFeature => "Windows Feature",
            Self::WindowsUpdate => "Windows Update",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UninstallerType {
    Msi,
    InnoSetup,
    Nsis,
    StoreApp,
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
        } else if lower.contains("remove-appxpackage") || lower.contains("appx") {
            Self::StoreApp
        } else {
            Self::Unknown
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Msi => "MSI",
            Self::InnoSetup => "Inno Setup",
            Self::Nsis => "NSIS",
            Self::StoreApp => "Store App",
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
    pub is_orphan: bool,
    pub rating: Option<u8>,
}

impl ApplicationEntry {
    pub fn can_uninstall(&self) -> bool {
        if self.is_protected {
            return false;
        }
        if self.is_system_component && !Self::is_windows_managed_source(self.source) {
            return false;
        }
        self.uninstall_string.is_some() || self.quiet_uninstall_string.is_some()
    }

    fn is_windows_managed_source(source: AppSource) -> bool {
        matches!(source, AppSource::WindowsFeature | AppSource::WindowsUpdate)
    }

    pub fn can_force_uninstall(&self) -> bool {
        if self.is_protected {
            return false;
        }
        if self.is_system_component && !Self::is_windows_managed_source(self.source) {
            return false;
        }

        if self.source == AppSource::Registry {
            return true;
        }

        self.install_location
            .as_deref()
            .is_some_and(crate::path_safety::is_safe_install_location)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JunkKind {
    File,
    Directory,
    RegistryKey,
}

impl JunkKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Directory => "Directory",
            Self::RegistryKey => "Registry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupLocation {
    RunKeyLocalMachine,
    RunKeyLocalMachineWow,
    RunKeyCurrentUser,
    StartupFolderUser,
    StartupFolderAllUsers,
}

impl StartupLocation {
    pub fn label(self) -> &'static str {
        match self {
            Self::RunKeyLocalMachine => "HKLM Run",
            Self::RunKeyLocalMachineWow => "HKLM Run (WOW)",
            Self::RunKeyCurrentUser => "HKCU Run",
            Self::StartupFolderUser => "User Startup",
            Self::StartupFolderAllUsers => "All Users Startup",
        }
    }

    pub fn is_registry(self) -> bool {
        matches!(
            self,
            Self::RunKeyLocalMachine | Self::RunKeyLocalMachineWow | Self::RunKeyCurrentUser
        )
    }

    pub fn requires_elevation(self) -> bool {
        matches!(
            self,
            Self::RunKeyLocalMachine | Self::RunKeyLocalMachineWow | Self::StartupFolderAllUsers
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupEntry {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: StartupLocation,
    pub enabled: bool,
    pub registry_hive: Option<String>,
    pub registry_path: Option<String>,
    pub registry_value_name: Option<String>,
    pub folder_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JunkItem {
    pub id: String,
    pub path: String,
    pub kind: JunkKind,
    pub size_estimate: Option<u64>,
    pub source_app: Option<String>,
    /// BCU-style confidence score (0-100); higher is safer to delete.
    pub confidence: u8,
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
