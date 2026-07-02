pub mod apps_view;
pub mod details;
pub mod handler;

use crate::config::Config;
use crate::models::ApplicationEntry;
use crate::scanner;
use crate::uninstall;
use anyhow::Result;
use apps_view::AppsView;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use details::DetailsView;
use handler::{filter_apps, Action, InputMode, KeyHandler};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::io::stdout;
use std::time::Duration;

pub struct TuiApp {
    apps: Vec<ApplicationEntry>,
    apps_view: AppsView,
    search_query: String,
    input_mode: InputMode,
    status_message: String,
    is_elevated: bool,
    config: Config,
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
        })
    }

    fn filtered_apps(&self) -> Vec<&ApplicationEntry> {
        filter_apps(&self.apps, &self.search_query)
    }

    fn selected_app(&self) -> Option<&ApplicationEntry> {
        let filtered = self.filtered_apps();
        let index = self.apps_view.selected_index();
        filtered.get(index).copied()
    }

    fn refresh_apps(&mut self) -> Result<()> {
        self.apps = scanner::scan_applications(&self.config)?;
        self.status_message = format!("Refreshed — {} applications", self.apps.len());
        Ok(())
    }

    fn uninstall_selected(&mut self) -> Result<()> {
        let app = self
            .selected_app()
            .ok_or_else(|| anyhow::anyhow!("no application selected"))?
            .clone();

        if !app.can_uninstall() {
            self.status_message = format!("Cannot uninstall: {}", app.display_name);
            return Ok(());
        }

        if !self.is_elevated {
            self.status_message = "Administrator privileges required for uninstall".to_string();
            return Ok(());
        }

        self.status_message = format!("Uninstalling {}...", app.display_name);

        match uninstall::uninstall_application(&app) {
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
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        loop {
            let filtered: Vec<ApplicationEntry> = self
                .filtered_apps()
                .into_iter()
                .map(|app| (*app).clone())
                .collect();
            let selected_index = self
                .apps_view
                .selected_index()
                .min(filtered.len().saturating_sub(1));
            let selected = filtered.get(selected_index);
            let search_query = self.search_query.clone();
            let status_message = self.status_message.clone();
            let is_elevated = self.is_elevated;

            terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(frame.area());

                self.apps_view.render(
                    frame,
                    chunks[0],
                    &filtered,
                    &search_query,
                    &status_message,
                    is_elevated,
                );

                DetailsView::render(frame, chunks[1], selected);
            })?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    let filtered_count = filtered.len();
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
                        Action::Uninstall => {
                            if let Err(e) = self.uninstall_selected() {
                                self.status_message = format!("Uninstall error: {e}");
                            }
                        }
                        Action::None => {}
                    }
                }
            }
        }

        Ok(())
    }
}
