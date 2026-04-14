use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let keys = if app.search_mode {
        vec![
            key_hint("Esc", "cancel"),
            key_hint("Enter", "apply"),
            key_hint("Up/Down", "navigate"),
        ]
    } else if app.done {
        vec![
            key_hint("q", "quit"),
            key_hint("Up/Down", "navigate"),
            key_hint("/", "search"),
        ]
    } else {
        vec![
            key_hint("q", "quit"),
            key_hint("Up/Down", "navigate"),
            key_hint("PgUp/PgDn", "scroll log"),
            key_hint("/", "search"),
        ]
    };

    let mut spans = Vec::new();
    for (i, group) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
        }
        spans.extend(group.clone());
    }

    let help = Paragraph::new(Line::from(spans));
    f.render_widget(help, area);
}

fn key_hint(key: &str, action: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {action}"), Style::default().fg(Color::DarkGray)),
    ]
}
