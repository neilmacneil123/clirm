use crate::models::JunkItem;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

pub struct LeftoversView {
    table_state: TableState,
}

impl LeftoversView {
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
        items: &[JunkItem],
        status_message: &str,
        scan_target: Option<&str>,
    ) {
        let title = match scan_target {
            Some(name) => format!(" Leftovers — {name} ({}) ", items.len()),
            None => format!(" Leftovers ({}) ", items.len()),
        };

        let header = Row::new(vec![
            Cell::from("Type"),
            Cell::from("Path"),
            Cell::from("Size"),
            Cell::from("App"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = items
            .iter()
            .map(|item| {
                let kind_style = match item.kind {
                    crate::models::JunkKind::RegistryKey => Style::default().fg(Color::Magenta),
                    crate::models::JunkKind::Directory => Style::default().fg(Color::Cyan),
                    crate::models::JunkKind::File => Style::default().fg(Color::Yellow),
                };

                Row::new(vec![
                    Cell::from(item.kind.label()).style(kind_style),
                    Cell::from(item.path.as_str()),
                    Cell::from(format_size(item.size_estimate)),
                    Cell::from(item.source_app.as_deref().unwrap_or("—")),
                ])
            })
            .collect();

        let visible = area.height.saturating_sub(3) as usize;
        self.ensure_visible(visible, items.len());

        let table = Table::new(
            rows,
            [
                ratatui::layout::Constraint::Length(10),
                ratatui::layout::Constraint::Percentage(55),
                ratatui::layout::Constraint::Length(10),
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
                    Span::raw("  |  q quit  l/Esc apps  j/k nav  d delete  s rescan"),
                ])),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}

impl Default for LeftoversView {
    fn default() -> Self {
        Self::new()
    }
}

fn format_size(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) if b >= 1_073_741_824 => format!("{:.1} GB", b as f64 / 1_073_741_824.0),
        Some(b) if b >= 1_048_576 => format!("{:.1} MB", b as f64 / 1_048_576.0),
        Some(b) if b >= 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        Some(b) => format!("{b} B"),
        None => "—".into(),
    }
}
