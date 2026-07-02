use crate::models::StartupEntry;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

pub struct StartupView {
    table_state: TableState,
}

impl StartupView {
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

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        entries: &[StartupEntry],
        status_message: &str,
    ) {
        let title = format!(" Startup Manager ({}) ", entries.len());

        let header = Row::new(vec![
            Cell::from("Name"),
            Cell::from("Location"),
            Cell::from("Command"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = entries
            .iter()
            .map(|entry| {
                let style = if entry.enabled {
                    Style::default()
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Row::new(vec![
                    Cell::from(entry.name.as_str()).style(style),
                    Cell::from(entry.location.label()),
                    Cell::from(truncate(&entry.command, 60)),
                ])
            })
            .collect();

        let visible = area.height.saturating_sub(3) as usize;
        self.ensure_visible(visible, entries.len());

        let table = Table::new(
            rows,
            [
                ratatui::layout::Constraint::Percentage(25),
                ratatui::layout::Constraint::Percentage(20),
                ratatui::layout::Constraint::Percentage(55),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_bottom(Line::from(vec![
                    Span::raw(status_message),
                    Span::raw("  |  d disable  r refresh  Esc back  q quit"),
                ])),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

impl Default for StartupView {
    fn default() -> Self {
        Self::new()
    }
}
