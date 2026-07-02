use crate::models::{AppSource, ApplicationEntry};
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterPreset {
    All,
    Registry,
    Store,
    Orphans,
    Large,
}

impl FilterPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Registry => "Registry",
            Self::Store => "Store",
            Self::Orphans => "Orphans",
            Self::Large => "Large (>500MB)",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Registry,
            Self::Registry => Self::Store,
            Self::Store => Self::Orphans,
            Self::Orphans => Self::Large,
            Self::Large => Self::All,
        }
    }
}

const LARGE_THRESHOLD: u64 = 500 * 1024 * 1024;

pub fn apply_preset(apps: &[ApplicationEntry], preset: FilterPreset) -> Vec<usize> {
    apps.iter()
        .enumerate()
        .filter(|(_, app)| match preset {
            FilterPreset::All => true,
            FilterPreset::Registry => app.source == AppSource::Registry,
            FilterPreset::Store => app.source == AppSource::Store,
            FilterPreset::Orphans => app.is_orphan,
            FilterPreset::Large => app
                .size_estimate
                .is_some_and(|s| s >= LARGE_THRESHOLD),
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn apply_search(
    apps: &[ApplicationEntry],
    indices: &[usize],
    query: &str,
    use_regex: bool,
) -> Vec<usize> {
    if query.is_empty() {
        return indices.to_vec();
    }

    if use_regex {
        let Ok(re) = Regex::new(query) else {
            return Vec::new();
        };
        return indices
            .iter()
            .copied()
            .filter(|&i| {
                let app = &apps[i];
                re.is_match(&app.display_name)
                    || app.publisher.as_ref().is_some_and(|p| re.is_match(p))
                    || app.version.as_ref().is_some_and(|v| re.is_match(v))
            })
            .collect();
    }

    let needle = query.to_lowercase();
    indices
        .iter()
        .copied()
        .filter(|&i| {
            let app = &apps[i];
            app.display_name.to_lowercase().contains(&needle)
                || app
                    .publisher
                    .as_ref()
                    .is_some_and(|p| p.to_lowercase().contains(&needle))
                || app
                    .version
                    .as_ref()
                    .is_some_and(|v| v.to_lowercase().contains(&needle))
        })
        .collect()
}

pub fn filter_visible(
    apps: &[ApplicationEntry],
    preset: FilterPreset,
    query: &str,
    use_regex: bool,
) -> Vec<usize> {
    let preset_indices = apply_preset(apps, preset);
    apply_search(apps, &preset_indices, query, use_regex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, ApplicationEntry, UninstallerType};

    fn app(source: AppSource, name: &str, size: Option<u64>, orphan: bool) -> ApplicationEntry {
        ApplicationEntry {
            id: name.into(),
            display_name: name.into(),
            version: None,
            publisher: None,
            source,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: size,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: orphan,
            rating: None,
        }
    }

    #[test]
    fn preset_store_filters() {
        let apps = vec![
            app(AppSource::Registry, "A", None, false),
            app(AppSource::Store, "B", None, false),
        ];
        assert_eq!(apply_preset(&apps, FilterPreset::Store), vec![1]);
    }

    #[test]
    fn regex_search_filters() {
        let apps = vec![app(AppSource::Registry, "FooBar", None, false)];
        let all = vec![0];
        assert_eq!(apply_search(&apps, &all, "^Foo", true), vec![0]);
    }
}