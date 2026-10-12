use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_confirm_dialog(frame: &mut Frame, area: Rect, app_name: &str) {
    let popup_width = area.width.min(60);
    let popup_height = 7;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Confirm Uninstall ");

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Uninstall "),
            Span::styled(
                app_name,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            ),
            Span::raw("?"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  y = yes    n / Esc = cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, popup);
}