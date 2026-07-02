pub mod apps_view;
pub mod confirm;
pub mod details;
pub mod handler;
mod terminal_guard;

use crate::config::Config;
use crate::models::ApplicationEntry;
use crate::scanner;
use crate::uninstall;
use anyhow::Result;
use apps_view::AppsView;
use crossterm::event::{self, Event};
use details::DetailsView;
use handler::{filter_apps, Action, InputMode, KeyHandler};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::time::Duration;
use terminal_guard::TerminalGuard;

pub struct TuiApp {
    apps: Vec<ApplicationEntry>,
    apps_view: AppsView,
    search_query: String,
    input_mode: InputMode,
    status_message: String,
    is_elevated: bool,
    config: Config,
    confirm_target: Option<ApplicationEntry>,
}

impl TuiApp {
    pub fn new(config: Config, is_elevated: bool) -> Result<Self> {
        let apps = scanner::scan_applications(&config)?;
        let count = apps.len();
        Ok(Self {
            apps,
            apps_view: AppsView::new(),
            search_query: String::new(),
            input_mode: InputMode::Normal,
            status_message: format!("Loaded {count} applications"),
            is_elevated,
            config,
            confirm_target: None,
        })
    }

    fn filtered_indices(&self) -> Vec<usize> {
        filter_apps(&self.apps, &self.search_query)
    }

    fn selected_app(&self) -> Option<&ApplicationEntry> {
        let indices = self.filtered_indices();
        let index = self.apps_view.selected_index();
        indices.get(index).map(|&i| &self.apps[i])
    }

    fn refresh_apps(&mut self) -> Result<()> {
        self.apps = scanner::scan_applications(&self.config)?;
        self.status_message = format!("Refreshed — {} applications", self.apps.len());
        Ok(())
    }

    fn request_uninstall(&mut self) {
        let Some(app) = self.selected_app().cloned() else {
            self.status_message = "No application selected".to_string();
            return;
        };

        if !app.can_uninstall() {
            self.status_message = format!("Cannot uninstall: {}", app.display_name);
            return;
        }

        if !self.is_elevated {
            self.status_message = "Administrator privileges required for uninstall".to_string();
            return;
        }

        self.confirm_target = Some(app);
        self.input_mode = InputMode::Confirm;
        self.status_message = "Confirm uninstall (y/n)".to_string();
    }

    fn execute_uninstall(&mut self, app: &ApplicationEntry) -> Result<()> {
        self.status_message = format!("Uninstalling {}...", app.display_name);

        match uninstall::uninstall_application(app) {
            Ok(()) => {
                self.status_message = format!("Uninstalled {}", app.display_name);
                self.refresh_apps()?;
            }
            Err(e) => {
                self.status_message = format!("Uninstall failed: {e}");
            }
        }

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        let mut guard = TerminalGuard::new()?;
        self.run_loop(&mut guard.terminal)
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        loop {
            let indices = self.filtered_indices();
            let filtered: Vec<&ApplicationEntry> =
                indices.iter().map(|&i| &self.apps[i]).collect();
            let selected_index = self
                .apps_view
                .selected_index()
                .min(filtered.len().saturating_sub(1));
            let selected = filtered.get(selected_index).copied();
            let search_query = self.search_query.clone();
            let status_message = self.status_message.clone();
            let is_elevated = self.is_elevated;
            let confirm_name = self.confirm_target.as_ref().map(|a| a.display_name.clone());
            let show_confirm = self.input_mode == InputMode::Confirm;

            terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(frame.area());

                self.apps_view.render(
                    frame,
                    chunks[0],
                    &filtered.iter().map(|app| (*app).clone()).collect::<Vec<_>>(),
                    &search_query,
                    &status_message,
                    is_elevated,
                );

                DetailsView::render(frame, chunks[1], selected);

                if show_confirm {
                    if let Some(name) = &confirm_name {
                        confirm::render_confirm_dialog(frame, frame.area(), name);
                    }
                }
            })?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    let filtered_count = indices.len();
                    let action = KeyHandler::handle(
                        key,
                        &mut self.input_mode,
                        &mut self.search_query,
                        &mut self.apps_view,
                        filtered_count,
                    );

                    match action {
                        Action::Quit => break,
                        Action::Refresh => {
                            if let Err(e) = self.refresh_apps() {
                                self.status_message = format!("Refresh failed: {e}");
                            }
                        }
                        Action::RequestUninstall => self.request_uninstall(),
                        Action::ConfirmUninstall => {
                            if let Some(app) = self.confirm_target.take() {
                                if let Err(e) = self.execute_uninstall(&app) {
                                    self.status_message = format!("Uninstall error: {e}");
                                }
                            }
                        }
                        Action::CancelUninstall => {
                            self.confirm_target = None;
                            self.status_message = "Uninstall cancelled".to_string();
                        }
                        Action::None => {}
                    }
                }
            }
        }

        Ok(())
    }
}