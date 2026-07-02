use crate::filter::bcul::BculList;
use crate::filter::{filter_visible, FilterPreset};
use crate::models::ApplicationEntry;
use crate::tui::apps_view::AppsView;
use crate::tui::leftovers_view::LeftoversView;
use crate::tui::startup_view::StartupView;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Apps,
    Leftovers,
    Startup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Confirm,
    BatchConfirm,
    ForceConfirm,
    LeftoverConfirm,
    StartupConfirm,
}

pub struct KeyHandler;

impl KeyHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn handle(
        key: KeyEvent,
        view: ViewMode,
        mode: &mut InputMode,
        search_query: &mut String,
        apps_view: &mut AppsView,
        leftovers_view: &mut LeftoversView,
        startup_view: &mut StartupView,
        filtered_count: usize,
        leftovers_count: usize,
        startup_count: usize,
    ) -> Action {
        match *mode {
            InputMode::Confirm
            | InputMode::BatchConfirm
            | InputMode::ForceConfirm
            | InputMode::LeftoverConfirm
            | InputMode::StartupConfirm => Self::handle_confirm(key, mode),
            InputMode::Search => Self::handle_search(key, mode, search_query, apps_view),
            InputMode::Normal => match view {
                ViewMode::Apps => Self::handle_normal(key, mode, apps_view, filtered_count),
                ViewMode::Leftovers => Self::handle_leftovers(key, leftovers_view, leftovers_count),
                ViewMode::Startup => Self::handle_startup(key, startup_view, startup_count),
            },
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
            KeyCode::Char('Z') => Action::RequestForceUninstall,
            KeyCode::Char('l') => Action::OpenLeftovers,
            KeyCode::Char(' ') => Action::ToggleSelection,
            KeyCode::Char('a') => Action::SelectAll,
            KeyCode::Char('f') => Action::CycleFilter,
            KeyCode::Char('R') => Action::ToggleRegex,
            KeyCode::Char('e') => Action::Export,
            KeyCode::Char('p') => Action::ToggleProtection,
            KeyCode::Char('S') => Action::OpenStartup,
            _ => Action::None,
        }
    }

    fn handle_startup(
        key: KeyEvent,
        startup_view: &mut StartupView,
        startup_count: usize,
    ) -> Action {
        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('S') | KeyCode::Esc => Action::CloseStartup,
            KeyCode::Char('j') | KeyCode::Down => {
                startup_view.move_down(startup_count);
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                startup_view.move_up();
                Action::None
            }
            KeyCode::Char('d') => Action::RequestDisableStartup,
            KeyCode::Char('r') => Action::RefreshStartup,
            _ => Action::None,
        }
    }

    fn handle_confirm(key: KeyEvent, mode: &mut InputMode) -> Action {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let action = match *mode {
                    InputMode::BatchConfirm => Action::ConfirmBatchUninstall,
                    InputMode::ForceConfirm => Action::ConfirmForceUninstall,
                    InputMode::LeftoverConfirm => Action::ConfirmDeleteLeftover,
                    InputMode::StartupConfirm => Action::ConfirmDisableStartup,
                    _ => Action::ConfirmUninstall,
                };
                *mode = InputMode::Normal;
                action
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                *mode = InputMode::Normal;
                Action::CancelUninstall
            }
            _ => Action::None,
        }
    }

    fn handle_leftovers(
        key: KeyEvent,
        leftovers_view: &mut LeftoversView,
        leftovers_count: usize,
    ) -> Action {
        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('l') | KeyCode::Esc => Action::CloseLeftovers,
            KeyCode::Char('j') | KeyCode::Down => {
                leftovers_view.move_down(leftovers_count);
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                leftovers_view.move_up();
                Action::None
            }
            KeyCode::Char('d') => Action::RequestDeleteLeftover,
            KeyCode::Char('s') => Action::RescanLeftovers,
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
    OpenLeftovers,
    CloseLeftovers,
    RescanLeftovers,
    RequestDeleteLeftover,
    ConfirmDeleteLeftover,
    RequestForceUninstall,
    ConfirmForceUninstall,
    OpenStartup,
    CloseStartup,
    RefreshStartup,
    RequestDisableStartup,
    ConfirmDisableStartup,
}

pub fn filtered_indices(
    apps: &[ApplicationEntry],
    preset: FilterPreset,
    query: &str,
    use_regex: bool,
    bcul: Option<&BculList>,
) -> Vec<usize> {
    filter_visible(apps, preset, query, use_regex, bcul)
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

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn handle_key(
        key: KeyEvent,
        view: ViewMode,
        mode: &mut InputMode,
        search_query: &mut String,
        apps_view: &mut AppsView,
        leftovers_view: &mut LeftoversView,
        startup_view: &mut StartupView,
        filtered_count: usize,
        leftovers_count: usize,
        startup_count: usize,
    ) -> Action {
        KeyHandler::handle(
            key,
            view,
            mode,
            search_query,
            apps_view,
            leftovers_view,
            startup_view,
            filtered_count,
            leftovers_count,
            startup_count,
        )
    }

    #[test]
    fn filtered_indices_all() {
        let apps = vec![sample_app("A"), sample_app("B")];
        assert_eq!(
            filtered_indices(&apps, FilterPreset::All, "", false, None),
            vec![0, 1]
        );
    }

    #[test]
    fn leftovers_q_quits() {
        let mut mode = InputMode::Normal;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('q')),
            ViewMode::Leftovers,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            0,
            0,
            0,
        );

        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn leftovers_esc_returns_to_apps() {
        let mut mode = InputMode::Normal;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Esc),
            ViewMode::Leftovers,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            0,
            0,
            0,
        );

        assert_eq!(action, Action::CloseLeftovers);
    }

    #[test]
    fn leftovers_d_requests_delete() {
        let mut mode = InputMode::Normal;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('d')),
            ViewMode::Leftovers,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            0,
            1,
            0,
        );

        assert_eq!(action, Action::RequestDeleteLeftover);
    }

    #[test]
    fn force_confirm_y_confirms() {
        let mut mode = InputMode::ForceConfirm;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('y')),
            ViewMode::Apps,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            1,
            0,
            0,
        );

        assert_eq!(action, Action::ConfirmForceUninstall);
        assert_eq!(mode, InputMode::Normal);
    }

    #[test]
    fn force_confirm_esc_cancels() {
        let mut mode = InputMode::ForceConfirm;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Esc),
            ViewMode::Apps,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            1,
            0,
            0,
        );

        assert_eq!(action, Action::CancelUninstall);
        assert_eq!(mode, InputMode::Normal);
    }

    #[test]
    fn leftover_confirm_does_not_leak_to_leftovers_actions() {
        let mut mode = InputMode::LeftoverConfirm;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('d')),
            ViewMode::Leftovers,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            0,
            1,
            0,
        );

        assert_eq!(action, Action::None);
    }

    #[test]
    fn leftover_confirm_y_deletes() {
        let mut mode = InputMode::LeftoverConfirm;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('y')),
            ViewMode::Leftovers,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            0,
            1,
            0,
        );

        assert_eq!(action, Action::ConfirmDeleteLeftover);
        assert_eq!(mode, InputMode::Normal);
    }

    #[test]
    fn apps_open_leftovers_shortcut() {
        let mut mode = InputMode::Normal;
        let mut search = String::new();
        let mut apps_view = AppsView::new();
        let mut leftovers_view = LeftoversView::new();
        let mut startup_view = StartupView::new();

        let action = handle_key(
            key_event(KeyCode::Char('l')),
            ViewMode::Apps,
            &mut mode,
            &mut search,
            &mut apps_view,
            &mut leftovers_view,
            &mut startup_view,
            1,
            0,
            0,
        );

        assert_eq!(action, Action::OpenLeftovers);
    }
}
