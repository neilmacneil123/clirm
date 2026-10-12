use crate::filter::{filter_visible, FilterPreset};
use crate::models::ApplicationEntry;
use crate::tui::apps_view::AppsView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Confirm,
    BatchConfirm,
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
            InputMode::Confirm | InputMode::BatchConfirm => Self::handle_confirm(key, mode),
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
            KeyCode::Char('X') => Action::RequestBatchUninstall,
            KeyCode::Char(' ') => Action::ToggleSelection,
            KeyCode::Char('a') => Action::SelectAll,
            KeyCode::Char('f') => Action::CycleFilter,
            KeyCode::Char('R') => Action::ToggleRegex,
            KeyCode::Char('e') => Action::Export,
            KeyCode::Char('p') => Action::ToggleProtection,
            _ => Action::None,
        }
    }

    fn handle_confirm(key: KeyEvent, mode: &mut InputMode) -> Action {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let batch = *mode == InputMode::BatchConfirm;
                *mode = InputMode::Normal;
                if batch {
                    Action::ConfirmBatchUninstall
                } else {
                    Action::ConfirmUninstall
                }
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
    RequestBatchUninstall,
    ConfirmUninstall,
    ConfirmBatchUninstall,
    CancelUninstall,
    ToggleSelection,
    SelectAll,
    CycleFilter,
    ToggleRegex,
    Export,
    ToggleProtection,
}

pub fn filtered_indices(
    apps: &[ApplicationEntry],
    preset: FilterPreset,
    query: &str,
    use_regex: bool,
) -> Vec<usize> {
    filter_visible(apps, preset, query, use_regex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, ApplicationEntry, UninstallerType};

    fn sample_app(name: &str) -> ApplicationEntry {
        ApplicationEntry {
            id: name.to_string(),
            display_name: name.to_string(),
            version: None,
            publisher: None,
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        }
    }

    #[test]
    fn filtered_indices_all() {
        let apps = vec![sample_app("A"), sample_app("B")];
        assert_eq!(filtered_indices(&apps, FilterPreset::All, "", false), vec![0, 1]);
    }
}