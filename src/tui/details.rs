use crate::models::ApplicationEntry;
use crate::ratings::RatingTier;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct DetailsView;

impl DetailsView {
    pub fn render(frame: &mut Frame, area: Rect, app: Option<&ApplicationEntry>) {
        let block = Block::default().borders(Borders::ALL).title(" Details ");

        let content = match app {
            Some(entry) => details_lines(entry),
            None => vec![Line::from("No application selected.")],
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

fn details_lines(app: &ApplicationEntry) -> Vec<Line<'static>> {
    let mut lines = vec![
        detail_line("Name", app.display_name.clone()),
        detail_line("Version", opt_str(&app.version)),
        detail_line("Publisher", opt_str(&app.publisher)),
        detail_line("Source", app.source.label().to_string()),
        detail_line("Type", app.uninstaller_type.label().to_string()),
        detail_line("Location", opt_str(&app.install_location)),
        detail_line("Install date", opt_str(&app.install_date)),
        detail_line("Size", format_size(app.size_estimate)),
        detail_line("Protected", bool_label(app.is_protected)),
        detail_line("System component", bool_label(app.is_system_component)),
        detail_line("Can uninstall", bool_label(app.can_uninstall())),
        detail_line(
            "Rating",
            app.rating
                .map(|r| format!("{} ({})", RatingTier::from_score(r).label(), r))
                .unwrap_or_else(|| "—".into()),
        ),
    ];

    if let Some(cmd) = &app.uninstall_string {
        lines.push(detail_line("Uninstall", cmd.clone()));
    }
    if let Some(cmd) = &app.quiet_uninstall_string {
        lines.push(detail_line("Quiet uninstall", cmd.clone()));
    }

    lines
}

fn detail_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label}: "),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(value),
    ])
}

fn opt_str(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "—".into())
}

fn bool_label(value: bool) -> String {
    if value {
        "Yes".into()
    } else {
        "No".into()
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
