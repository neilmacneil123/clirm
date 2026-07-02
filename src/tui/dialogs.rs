use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_leftover_confirm(frame: &mut Frame, area: Rect, path: &str, kind: &str) {
    let popup = centered_rect(65, 24, area);
    frame.render_widget(Clear, popup);
    let text = format!(
        "Delete leftover {kind}?\n\n{path}\n\nThis action is irreversible.\n\nPress Y to confirm, N to cancel."
    );
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Delete Leftover ")
                .style(Style::default().add_modifier(Modifier::BOLD)),
        );
    frame.render_widget(paragraph, popup);
}

pub fn render_force_confirm(frame: &mut Frame, area: Rect, app_name: &str) {
    let popup = centered_rect(60, 22, area);
    frame.render_widget(Clear, popup);
    let text = format!(
        "FORCE uninstall {app_name}?\n\nThis deletes files and registry entries without running the uninstaller.\n\nPress Y to confirm, N to cancel."
    );
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Force Uninstall ")
                .style(Style::default().add_modifier(Modifier::BOLD)),
        );
    frame.render_widget(paragraph, popup);
}

pub fn render_batch_confirm(frame: &mut Frame, area: Rect, count: usize) {
    let popup = centered_rect(60, 20, area);
    frame.render_widget(Clear, popup);
    let text =
        format!("Uninstall {count} selected application(s)?\n\nPress Y to confirm, N to cancel.");
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
