use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::UiState;

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    let mut keys = if state.search_mode {
        vec![
            key_hint("Esc", "cancel"),
            key_hint("Enter", "apply"),
            key_hint("j/k", "navigate"),
        ]
    } else if state.filter_active() {
        vec![
            key_hint("Esc", "clear filter"),
            key_hint("q", "quit"),
            key_hint("j/k", "navigate"),
            key_hint("/", "search"),
        ]
    } else if state.done {
        vec![
            key_hint("q", "quit"),
            key_hint("j/k", "navigate"),
            key_hint("/", "search"),
        ]
    } else {
        vec![
            key_hint("q", "quit"),
            if state.in_merge_mode() {
                key_hint("j/k", "list")
            } else {
                key_hint("j/k", "navigate")
            },
            key_hint("PgUp/PgDn", "scroll"),
            key_hint("Home/End", "log"),
            key_hint("/", "search"),
        ]
    };

    if !state.log_follow && !state.search_mode {
        keys.push(key_hint("End", "follow"));
    }

    let mut spans = Vec::new();
    for (i, group) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
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
