use crate::filter::FilterPreset;
use crate::models::ApplicationEntry;
use crate::ratings::RatingTier;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};
use std::collections::HashSet;

pub struct AppsView {
    table_state: TableState,
    selected_ids: HashSet<String>,
}

impl AppsView {
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            selected_ids: HashSet::new(),
        }
    }

    pub fn select(&mut self, index: usize) {
        self.table_state.select(Some(index));
    }

    pub fn selected_index(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selected_ids.contains(id)
    }

    pub fn toggle_selection(&mut self, id: &str) {
        if self.selected_ids.contains(id) {
            self.selected_ids.remove(id);
        } else {
            self.selected_ids.insert(id.to_string());
        }
    }

    pub fn select_all(&mut self, apps: &[ApplicationEntry]) {
        if self.selected_ids.len() == apps.len() {
            self.selected_ids.clear();
        } else {
            self.selected_ids = apps.iter().map(|a| a.id.clone()).collect();
        }
    }

    pub fn selected_apps<'a>(&self, all: &'a [ApplicationEntry]) -> Vec<&'a ApplicationEntry> {
        all.iter()
            .filter(|a| self.selected_ids.contains(&a.id))
            .collect()
    }

    pub fn move_down(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let current = self.selected_index().min(max - 1);
        if current + 1 < max {
            self.table_state.select(Some(current + 1));
        }
    }

    pub fn move_up(&mut self) {
        let current = self.selected_index();
        if current > 0 {
            self.table_state.select(Some(current - 1));
        }
    }

    pub fn ensure_visible(&mut self, visible_rows: usize, total: usize) {
        if total == 0 {
            self.table_state.select(None);
            *self.table_state.offset_mut() = 0;
            return;
        }

        let selected = self.selected_index().min(total - 1);
        self.table_state.select(Some(selected));

        if visible_rows == 0 {
            return;
        }

        let offset = self.table_state.offset();
        if selected < offset {
            *self.table_state.offset_mut() = selected;
        } else if selected >= offset.saturating_add(visible_rows) {
            *self.table_state.offset_mut() =
                selected.saturating_sub(visible_rows.saturating_sub(1));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        apps: &[ApplicationEntry],
        search_query: &str,
        status_message: &str,
        is_elevated: bool,
        preset: FilterPreset,
        use_regex: bool,
    ) {
        let regex_label = if use_regex { "regex:on" } else { "regex:off" };
        let title = if search_query.is_empty() {
            format!(
                " Applications ({}) — {} — {} ",
                apps.len(),
                preset.label(),
                regex_label
            )
        } else {
            format!(
                " Applications ({}) — {} — {regex_label} — filter: {search_query} ",
                apps.len(),
                preset.label()
            )
        };

        let header = Row::new(vec![
            Cell::from(" "),
            Cell::from("Name"),
            Cell::from("Version"),
            Cell::from("Rating"),
            Cell::from("Source"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = apps
            .iter()
            .map(|app| {
                let name_style = if app.is_protected || app.is_system_component {
                    Style::default().fg(Color::DarkGray)
                } else if !app.can_uninstall() {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };

                let rating_label = app
                    .rating
                    .map(|r| format!("{} ({})", RatingTier::from_score(r).label(), r))
                    .unwrap_or_else(|| "—".into());
                let rating_style = match app.rating.map(RatingTier::from_score) {
                    Some(RatingTier::Safe) => Style::default().fg(Color::Green),
                    Some(RatingTier::Moderate) => Style::default().fg(Color::Yellow),
                    Some(RatingTier::Risky) => Style::default().fg(Color::Red),
                    _ => Style::default().fg(Color::DarkGray),
                };

                let mark = if self.is_selected(&app.id) {
                    "[x]"
                } else {
                    "[ ]"
                };

                Row::new(vec![
                    Cell::from(mark),
                    Cell::from(app.display_name.as_str()).style(name_style),
                    Cell::from(app.version.as_deref().unwrap_or("—")),
                    Cell::from(rating_label).style(rating_style),
                    Cell::from(app.source.label()),
                ])
            })
            .collect();

        let visible = area.height.saturating_sub(3) as usize;
        self.ensure_visible(visible, apps.len());

        let table = Table::new(
            rows,
            [
                ratatui::layout::Constraint::Length(4),
                ratatui::layout::Constraint::Percentage(40),
                ratatui::layout::Constraint::Percentage(15),
                ratatui::layout::Constraint::Percentage(18),
                ratatui::layout::Constraint::Percentage(18),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_bottom(Line::from(vec![
                    Span::raw(status_message),
                    Span::raw("  |  "),
                    Span::styled(
                        if is_elevated { "Admin" } else { "Not elevated" },
                        Style::default().fg(if is_elevated {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                    Span::raw("  |  Space select  a all  x uninstall  X batch  Z force  l leftovers  S startup  f filter  R regex  e export  p protect"),
                ])),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}

impl Default for AppsView {
    fn default() -> Self {
        Self::new()
    }
}
