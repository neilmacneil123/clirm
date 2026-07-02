use crate::models::ApplicationEntry;
use crate::tui::apps_view::AppsView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Confirm,
}

pub struct KeyHandler;

impl KeyHandler {
    pub fn handle(
        key: KeyEvent,
        mode: &mut InputMode,
        search_query: &mut String,
        apps_view: &mut AppsView,
        filtered_count: usize,
    ) -> Action {
        match *mode {
            InputMode::Confirm => Self::handle_confirm(key, mode),
            InputMode::Search => Self::handle_search(key, mode, search_query, apps_view),
            InputMode::Normal => Self::handle_normal(key, mode, apps_view, filtered_count),
        }
    }

    fn handle_normal(
        key: KeyEvent,
        mode: &mut InputMode,
        apps_view: &mut AppsView,
        filtered_count: usize,
    ) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('j') | KeyCode::Down => {
                apps_view.move_down(filtered_count);
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                apps_view.move_up();
                Action::None
            }
            KeyCode::Char('/') => {
                *mode = InputMode::Search;
                Action::None
            }
            KeyCode::Char('r') => Action::Refresh,
            KeyCode::Char('x') => Action::RequestUninstall,
            _ => Action::None,
        }
    }

    fn handle_confirm(key: KeyEvent, mode: &mut InputMode) -> Action {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                *mode = InputMode::Normal;
                Action::ConfirmUninstall
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                *mode = InputMode::Normal;
                Action::CancelUninstall
            }
            _ => Action::None,
        }
    }

    fn handle_search(
        key: KeyEvent,
        mode: &mut InputMode,
        search_query: &mut String,
        apps_view: &mut AppsView,
    ) -> Action {
        match key.code {
            KeyCode::Esc => {
                *mode = InputMode::Normal;
                Action::None
            }
            KeyCode::Enter => {
                *mode = InputMode::Normal;
                Action::None
            }
            KeyCode::Backspace => {
                search_query.pop();
                apps_view.select(0);
                Action::None
            }
            KeyCode::Char(_) if key.modifiers.contains(KeyModifiers::CONTROL) => Action::None,
            KeyCode::Char(c) => {
                search_query.push(c);
                apps_view.select(0);
                Action::None
            }
            _ => Action::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Refresh,
    RequestUninstall,
    ConfirmUninstall,
    CancelUninstall,
}

pub fn filter_apps(apps: &[ApplicationEntry], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..apps.len()).collect();
    }

    let needle = query.to_lowercase();
    apps.iter()
        .enumerate()
        .filter(|(_, app)| {
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
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, ApplicationEntry, UninstallerType};

    fn sample_app(name: &str, publisher: Option<&str>, version: Option<&str>) -> ApplicationEntry {
        ApplicationEntry {
            id: name.to_string(),
            display_name: name.to_string(),
            version: version.map(str::to_string),
            publisher: publisher.map(str::to_string),
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            rating: None,
        }
    }

    #[test]
    fn filter_apps_empty_query_returns_all_indices() {
        let apps = vec![
            sample_app("Alpha", None, None),
            sample_app("Beta", None, None),
        ];
        assert_eq!(filter_apps(&apps, ""), vec![0, 1]);
    }

    #[test]
    fn filter_apps_matches_display_name() {
        let apps = vec![
            sample_app("Alpha App", None, None),
            sample_app("Beta App", None, None),
        ];
        assert_eq!(filter_apps(&apps, "alpha"), vec![0]);
    }

    #[test]
    fn filter_apps_matches_publisher_and_version() {
        let apps = vec![
            sample_app("One", Some("Acme Corp"), None),
            sample_app("Two", None, Some("2.0.0")),
        ];
        assert_eq!(filter_apps(&apps, "acme"), vec![0]);
        assert_eq!(filter_apps(&apps, "2.0"), vec![1]);
    }
}