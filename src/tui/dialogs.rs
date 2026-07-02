use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_batch_confirm(frame: &mut Frame, area: Rect, count: usize) {
    let popup = centered_rect(60, 20, area);
    frame.render_widget(Clear, popup);
    let text = format!(
        "Uninstall {count} selected application(s)?\n\nPress Y to confirm, N to cancel."
    );
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Batch Uninstall ")
                .style(Style::default().add_modifier(Modifier::BOLD)),
        );
    frame.render_widget(paragraph, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}