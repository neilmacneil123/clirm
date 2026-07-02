use crate::models::ApplicationEntry;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

pub struct AppsView {
    table_state: TableState,
}

impl AppsView {
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
        }
    }

    pub fn select(&mut self, index: usize) {
        self.table_state.select(Some(index));
    }

    pub fn selected_index(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn move_down(&mut self, max: usize) {
        let current = self.selected_index();
        if current + 1 < max {
            self.table_state.select(Some(current + 1));
        }
    }

    pub fn move_up(&mut self) {
        let current = self.selected_index();
        if current > 0 {
            self.table_state.select(Some(current.saturating_sub(1)));
        }
    }

    pub fn ensure_visible(&mut self, visible_rows: usize, total: usize) {
        if total == 0 {
            self.table_state.select(None);
            return;
        }

        let selected = self.selected_index().min(total - 1);
        self.table_state.select(Some(selected));
        let _ = visible_rows;
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        apps: &[ApplicationEntry],
        search_query: &str,
        status_message: &str,
        is_elevated: bool,
    ) {
        let title = if search_query.is_empty() {
            format!(" Applications ({}) ", apps.len())
        } else {
            format!(" Applications ({}) — filter: {search_query} ", apps.len())
        };

        let header = Row::new(vec![
            Cell::from("Name"),
            Cell::from("Version"),
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

                Row::new(vec![
                    Cell::from(app.display_name.as_str()).style(name_style),
                    Cell::from(app.version.as_deref().unwrap_or("—")),
                    Cell::from(app.source.label()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                ratatui::layout::Constraint::Percentage(55),
                ratatui::layout::Constraint::Percentage(20),
                ratatui::layout::Constraint::Percentage(25),
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
                    Span::raw("  |  / search  x uninstall  r refresh  q quit"),
                ])),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let visible = area.height.saturating_sub(3) as usize;
        self.ensure_visible(visible, apps.len());

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}

impl Default for AppsView {
    fn default() -> Self {
        Self::new()
    }
}
