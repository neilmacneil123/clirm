use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

const MIN_PATH_COMPONENTS: usize = 4;

/// Returns true when `location` is non-empty and safe for force-uninstall deletion.
pub fn is_safe_install_location(location: &str) -> bool {
    validate_deletable_path(Path::new(location.trim())).is_ok()
}

/// Validate that a filesystem path is safe to delete.
pub fn validate_deletable_path(path: &Path) -> Result<()> {
    let trimmed = path
        .to_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("refusing to delete empty path"))?;

    if trimmed != path.to_str().unwrap_or("") {
        bail!("refusing to delete path with leading or trailing whitespace");
    }

    let normalized = normalize_path(path);

    if is_blocked_system_path(&normalized) {
        bail!(
            "refusing to delete protected system path: {}",
            normalized.display()
        );
    }

    if path_component_count(&normalized) < MIN_PATH_COMPONENTS {
        bail!(
            "refusing to delete path with insufficient depth: {}",
            normalized.display()
        );
    }

    if !is_under_allowed_root(&normalized) {
        bail!(
            "refusing to delete path outside allowed locations: {}",
            normalized.display()
        );
    }

    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn path_component_count(path: &Path) -> usize {
    path.components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count()
        .saturating_add(1)
}

fn is_blocked_system_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_ascii_lowercase();

    const BLOCKED_EXACT: &[&str] = &[
        r"c:\",
        r"c:\windows",
        r"c:\program files",
        r"c:\program files (x86)",
        r"c:\programdata",
        r"c:\users",
    ];

    if BLOCKED_EXACT.iter().any(|blocked| path_str == *blocked) {
        return true;
    }

    const BLOCKED_PREFIXES: &[&str] = &[r"c:\windows\", r"c:\windows\system32\"];

    BLOCKED_PREFIXES
        .iter()
        .any(|prefix| path_str.starts_with(prefix))
}

fn is_under_allowed_root(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_ascii_lowercase();
    allowed_roots()
        .into_iter()
        .any(|root| path_str.starts_with(&root))
}

fn allowed_roots() -> Vec<String> {
    let mut roots = Vec::new();
    for var in [
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "LOCALAPPDATA",
        "APPDATA",
        "PROGRAMDATA",
    ] {
        if let Ok(value) = std::env::var(var) {
            roots.push(value.to_ascii_lowercase());
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_path() {
        assert!(validate_deletable_path(Path::new("")).is_err());
    }

    #[test]
    fn rejects_whitespace_only_path() {
        assert!(validate_deletable_path(Path::new("   ")).is_err());
    }

    #[test]
    fn rejects_program_files_root() {
        assert!(validate_deletable_path(Path::new(r"C:\Program Files")).is_err());
    }

    #[test]
    fn rejects_windows_directory() {
        assert!(validate_deletable_path(Path::new(r"C:\Windows")).is_err());
    }

    #[test]
    fn allows_nested_program_files_path() {
        let path = Path::new(r"C:\Program Files\Vendor\AppName");
        if allowed_roots().is_empty() {
            return;
        }
        assert!(validate_deletable_path(path).is_ok());
    }

    #[test]
    fn is_safe_install_location_rejects_empty() {
        assert!(!is_safe_install_location(""));
        assert!(!is_safe_install_location("   "));
    }
}
