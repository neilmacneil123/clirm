use crate::models::ApplicationEntry;
use crate::tui::apps_view::AppsView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
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
            KeyCode::Char('x') => Action::Uninstall,
            KeyCode::Enter => Action::Uninstall,
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
    Uninstall,
}

pub fn filter_apps<'a>(apps: &'a [ApplicationEntry], query: &str) -> Vec<&'a ApplicationEntry> {
    if query.is_empty() {
        return apps.iter().collect();
    }

    let needle = query.to_lowercase();
    apps.iter()
        .filter(|app| {
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
