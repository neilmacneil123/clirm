pub mod apps_view;
pub mod confirm;
pub mod details;
pub mod dialogs;
pub mod handler;
pub mod leftovers_view;
mod terminal_guard;

use crate::config::Config;
use crate::export;
use crate::filter::FilterPreset;
use crate::junk::{delete_junk_item, scan_for_app, scan_orphan_leftovers};
use crate::models::{ApplicationEntry, JunkItem};
use crate::protection::ProtectionList;
use crate::scanner;
use crate::uninstall::{force_uninstall, UninstallQueue};
use anyhow::Result;
use apps_view::AppsView;
use crossterm::event::{self, Event};
use details::DetailsView;
use handler::{filtered_indices, Action, InputMode, KeyHandler, ViewMode};
use leftovers_view::LeftoversView;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::time::Duration;
use terminal_guard::TerminalGuard;

pub struct TuiApp {
    apps: Vec<ApplicationEntry>,
    apps_view: AppsView,
    leftovers_view: LeftoversView,
    leftovers: Vec<JunkItem>,
    leftovers_target: Option<String>,
    leftover_confirm_target: Option<JunkItem>,
    view_mode: ViewMode,
    search_query: String,
    input_mode: InputMode,
    status_message: String,
    is_elevated: bool,
    config: Config,
    protection: ProtectionList,
    confirm_target: Option<ApplicationEntry>,
    filter_preset: FilterPreset,
    use_regex: bool,
    uninstall_queue: Option<UninstallQueue>,
}

impl TuiApp {
    pub fn new(config: Config, protection: ProtectionList, is_elevated: bool) -> Result<Self> {
        let apps = scanner::scan_applications(&config, &protection)?;
        let count = apps.len();
        Ok(Self {
            apps,
            apps_view: AppsView::new(),
            leftovers_view: LeftoversView::new(),
            leftovers: Vec::new(),
            leftovers_target: None,
            leftover_confirm_target: None,
            view_mode: ViewMode::Apps,
            search_query: String::new(),
            input_mode: InputMode::Normal,
            status_message: format!("Loaded {count} applications"),
            is_elevated,
            config,
            protection,
            confirm_target: None,
            filter_preset: FilterPreset::All,
            use_regex: false,
            uninstall_queue: None,
        })
    }

    fn filtered_indices(&self) -> Vec<usize> {
        filtered_indices(
            &self.apps,
            self.filter_preset,
            &self.search_query,
            self.use_regex,
        )
    }

    fn selected_app(&self) -> Option<&ApplicationEntry> {
        let indices = self.filtered_indices();
        let index = self.apps_view.selected_index();
        indices.get(index).map(|&i| &self.apps[i])
    }

    fn refresh_apps(&mut self) -> Result<()> {
        self.apps = scanner::scan_applications(&self.config, &self.protection)?;
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

    fn request_batch_uninstall(&mut self) {
        if !self.is_elevated {
            self.status_message =
                "Administrator privileges required for batch uninstall".to_string();
            return;
        }

        let selected: Vec<ApplicationEntry> = self
            .apps_view
            .selected_apps(&self.apps)
            .into_iter()
            .filter(|a| a.can_uninstall())
            .cloned()
            .collect();

        if selected.is_empty() {
            self.status_message = "No uninstallable apps selected (use Space)".to_string();
            return;
        }

        self.uninstall_queue = Some(UninstallQueue::new(selected));
        self.input_mode = InputMode::BatchConfirm;
        let count = self
            .uninstall_queue
            .as_ref()
            .map(|q| q.total())
            .unwrap_or(0);
        self.status_message = format!("Confirm batch uninstall of {count} apps (y/n)");
    }

    fn execute_uninstall(&mut self, app: &ApplicationEntry) -> Result<()> {
        self.status_message = format!("Uninstalling {}...", app.display_name);

        match crate::uninstall::uninstall_application(app) {
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

    fn start_batch_uninstall(&mut self) {
        if let Some(queue) = &mut self.uninstall_queue {
            if let Some(app) = queue.current() {
                self.status_message = format!(
                    "Batch uninstall {}/{}: {}",
                    queue.done() + 1,
                    queue.total(),
                    app.display_name
                );
            }
        }
    }

    fn advance_batch(&mut self) -> Result<()> {
        if let Some(queue) = &mut self.uninstall_queue {
            queue.advance()?;
            if queue.is_finished() {
                self.status_message = format!(
                    "Batch complete: {} ok, {} failed",
                    queue.succeeded(),
                    queue.failed()
                );
                self.uninstall_queue = None;
                self.refresh_apps()?;
            } else if let Some(err) = queue.last_error() {
                self.status_message = format!("Batch error: {err}");
            }
        }
        Ok(())
    }

    fn toggle_protection(&mut self) -> Result<()> {
        let Some(app) = self.selected_app().cloned() else {
            self.status_message = "No application selected".to_string();
            return Ok(());
        };

        let now_protected = self.protection.toggle(&app.id);
        self.protection.save()?;
        self.refresh_apps()?;

        self.status_message = if now_protected {
            format!("Protected: {}", app.display_name)
        } else {
            format!("Unprotected: {}", app.display_name)
        };
        Ok(())
    }

    fn leftovers_target_display_name(&self) -> Option<String> {
        self.leftovers_target
            .as_ref()
            .and_then(|id| self.apps.iter().find(|a| &a.id == id))
            .map(|app| app.display_name.clone())
    }

    fn open_leftovers(&mut self) {
        if let Some(app) = self.selected_app().cloned() {
            match scan_for_app(&app) {
                Ok(items) => {
                    let count = items.len();
                    self.leftovers = items;
                    self.leftovers_target = Some(app.id.clone());
                    self.view_mode = ViewMode::Leftovers;
                    self.leftovers_view.select(0);
                    self.status_message =
                        format!("Found {count} leftover item(s) for {}", app.display_name);
                }
                Err(e) => {
                    self.status_message = format!("Leftover scan failed: {e}");
                }
            }
            return;
        }

        match scan_orphan_leftovers(&self.apps) {
            Ok(items) => {
                let count = items.len();
                self.leftovers = items;
                self.leftovers_target = None;
                self.view_mode = ViewMode::Leftovers;
                self.leftovers_view.select(0);
                self.status_message = format!("Found {count} orphan leftover item(s)");
            }
            Err(e) => {
                self.status_message = format!("Orphan scan failed: {e}");
            }
        }
    }

    fn rescan_leftovers(&mut self) {
        if let Some(id) = self.leftovers_target.clone() {
            let app = self.apps.iter().find(|a| a.id == id).cloned();
            if let Some(app) = app {
                match scan_for_app(&app) {
                    Ok(items) => {
                        self.leftovers = items;
                        self.status_message = format!(
                            "Rescanned {} — {} item(s)",
                            app.display_name,
                            self.leftovers.len()
                        );
                    }
                    Err(e) => self.status_message = format!("Rescan failed: {e}"),
                }
                return;
            }
        }

        match scan_orphan_leftovers(&self.apps) {
            Ok(items) => {
                self.leftovers = items;
                self.status_message =
                    format!("Rescanned orphans — {} item(s)", self.leftovers.len());
            }
            Err(e) => self.status_message = format!("Rescan failed: {e}"),
        }
    }

    fn request_delete_leftover(&mut self) {
        let index = self.leftovers_view.selected_index();
        let Some(item) = self.leftovers.get(index).cloned() else {
            self.status_message = "No leftover selected".to_string();
            return;
        };

        if !self.is_elevated {
            self.status_message =
                "Administrator privileges required to delete leftovers".to_string();
            return;
        }

        self.leftover_confirm_target = Some(item);
        self.input_mode = InputMode::LeftoverConfirm;
        self.status_message = "Confirm leftover deletion (y/n)".to_string();
    }

    fn execute_delete_leftover(&mut self) {
        let Some(item) = self.leftover_confirm_target.take() else {
            return;
        };

        match delete_junk_item(&item) {
            Ok(()) => {
                self.leftovers.retain(|j| j.id != item.id);
                self.status_message = format!("Deleted leftover: {}", item.path);
                if self.leftovers_view.selected_index() >= self.leftovers.len()
                    && !self.leftovers.is_empty()
                {
                    self.leftovers_view.select(self.leftovers.len() - 1);
                }
            }
            Err(e) => self.status_message = format!("Delete failed: {e}"),
        }
    }

    fn request_force_uninstall(&mut self) {
        let Some(app) = self.selected_app().cloned() else {
            self.status_message = "No application selected".to_string();
            return;
        };

        if !app.can_force_uninstall() {
            self.status_message = format!("Cannot force uninstall: {}", app.display_name);
            return;
        }

        if !self.is_elevated {
            self.status_message =
                "Administrator privileges required for force uninstall".to_string();
            return;
        }

        self.confirm_target = Some(app);
        self.input_mode = InputMode::ForceConfirm;
        self.status_message = "Confirm FORCE uninstall (y/n)".to_string();
    }

    fn execute_force_uninstall(&mut self, app: &ApplicationEntry) -> Result<()> {
        self.status_message = format!("Force uninstalling {}...", app.display_name);

        match force_uninstall(app) {
            Ok(()) => {
                self.status_message = format!("Force uninstalled {}", app.display_name);
                self.refresh_apps()?;
            }
            Err(e) => self.status_message = format!("Force uninstall failed: {e}"),
        }

        Ok(())
    }

    fn export_visible(&mut self) -> Result<()> {
        let indices = self.filtered_indices();
        let visible: Vec<ApplicationEntry> =
            indices.iter().map(|&i| self.apps[i].clone()).collect();

        let csv_path = export::default_export_path("csv")?;
        let json_path = export::default_export_path("json")?;
        export::export_csv(&visible, &csv_path)?;
        export::export_json(&visible, &json_path)?;

        self.status_message = format!(
            "Exported {} apps to {} and {}",
            visible.len(),
            csv_path.display(),
            json_path.display()
        );
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
            if self.uninstall_queue.is_some() && self.input_mode == InputMode::Normal {
                self.advance_batch()?;
            }

            let indices = self.filtered_indices();
            let filtered: Vec<&ApplicationEntry> = indices.iter().map(|&i| &self.apps[i]).collect();
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
            let show_batch = self.input_mode == InputMode::BatchConfirm;
            let batch_count = self
                .uninstall_queue
                .as_ref()
                .map(|q| q.total())
                .unwrap_or(0);
            let preset = self.filter_preset;
            let use_regex = self.use_regex;
            let view_mode = self.view_mode;
            let leftovers = self.leftovers.clone();
            let leftovers_target = self.leftovers_target_display_name();
            let show_force = self.input_mode == InputMode::ForceConfirm;
            let show_leftover_confirm = self.input_mode == InputMode::LeftoverConfirm;
            let leftover_confirm = self.leftover_confirm_target.clone();

            terminal.draw(|frame| match view_mode {
                ViewMode::Apps => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                        .split(frame.area());

                    let visible_apps: Vec<ApplicationEntry> =
                        filtered.iter().map(|a| (*a).clone()).collect();

                    self.apps_view.render(
                        frame,
                        chunks[0],
                        &visible_apps,
                        &search_query,
                        &status_message,
                        is_elevated,
                        preset,
                        use_regex,
                    );

                    DetailsView::render(frame, chunks[1], selected);

                    if show_confirm {
                        if let Some(name) = &confirm_name {
                            confirm::render_confirm_dialog(frame, frame.area(), name);
                        }
                    } else if show_batch {
                        dialogs::render_batch_confirm(frame, frame.area(), batch_count);
                    } else if show_force {
                        if let Some(name) = &confirm_name {
                            dialogs::render_force_confirm(frame, frame.area(), name);
                        }
                    }
                }
                ViewMode::Leftovers => {
                    self.leftovers_view.render(
                        frame,
                        frame.area(),
                        &leftovers,
                        &status_message,
                        leftovers_target.as_deref(),
                    );

                    if show_leftover_confirm {
                        if let Some(item) = &leftover_confirm {
                            dialogs::render_leftover_confirm(
                                frame,
                                frame.area(),
                                &item.path,
                                item.kind.label(),
                            );
                        }
                    }
                }
            })?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    let filtered_count = indices.len();
                    let action = KeyHandler::handle(
                        key,
                        self.view_mode,
                        &mut self.input_mode,
                        &mut self.search_query,
                        &mut self.apps_view,
                        &mut self.leftovers_view,
                        filtered_count,
                        self.leftovers.len(),
                    );

                    match action {
                        Action::Quit => break,
                        Action::Refresh => {
                            if let Err(e) = self.refresh_apps() {
                                self.status_message = format!("Refresh failed: {e}");
                            }
                        }
                        Action::RequestUninstall => self.request_uninstall(),
                        Action::RequestBatchUninstall => self.request_batch_uninstall(),
                        Action::ConfirmUninstall => {
                            if let Some(app) = self.confirm_target.take() {
                                if let Err(e) = self.execute_uninstall(&app) {
                                    self.status_message = format!("Uninstall error: {e}");
                                }
                            }
                        }
                        Action::ConfirmBatchUninstall => {
                            self.input_mode = InputMode::Normal;
                            self.start_batch_uninstall();
                        }
                        Action::CancelUninstall => {
                            self.confirm_target = None;
                            self.leftover_confirm_target = None;
                            self.uninstall_queue = None;
                            self.status_message = "Cancelled".to_string();
                        }
                        Action::ToggleSelection => {
                            let id = self.selected_app().map(|a| a.id.clone());
                            if let Some(id) = id {
                                self.apps_view.toggle_selection(&id);
                            }
                        }
                        Action::SelectAll => {
                            let visible: Vec<ApplicationEntry> =
                                indices.iter().map(|&i| self.apps[i].clone()).collect();
                            self.apps_view.select_all(&visible);
                        }
                        Action::CycleFilter => {
                            self.filter_preset = self.filter_preset.next();
                            self.apps_view.select(0);
                            self.status_message = format!("Filter: {}", self.filter_preset.label());
                        }
                        Action::ToggleRegex => {
                            self.use_regex = !self.use_regex;
                            self.status_message = if self.use_regex {
                                "Regex search enabled".to_string()
                            } else {
                                "Regex search disabled".to_string()
                            };
                        }
                        Action::Export => {
                            if let Err(e) = self.export_visible() {
                                self.status_message = format!("Export failed: {e}");
                            }
                        }
                        Action::ToggleProtection => {
                            if let Err(e) = self.toggle_protection() {
                                self.status_message = format!("Protection error: {e}");
                            }
                        }
                        Action::OpenLeftovers => self.open_leftovers(),
                        Action::CloseLeftovers => {
                            self.view_mode = ViewMode::Apps;
                            self.status_message = "Returned to applications".to_string();
                        }
                        Action::RescanLeftovers => self.rescan_leftovers(),
                        Action::RequestDeleteLeftover => self.request_delete_leftover(),
                        Action::ConfirmDeleteLeftover => self.execute_delete_leftover(),
                        Action::RequestForceUninstall => self.request_force_uninstall(),
                        Action::ConfirmForceUninstall => {
                            if let Some(app) = self.confirm_target.take() {
                                if let Err(e) = self.execute_force_uninstall(&app) {
                                    self.status_message = format!("Force uninstall error: {e}");
                                }
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
