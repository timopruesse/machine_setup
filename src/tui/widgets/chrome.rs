use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::tui::theme::Theme;

pub fn key_hint(theme: &Theme, key: &str, action: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {action}"), Style::default().fg(theme.muted)),
    ]
}

pub fn hint_separator(theme: &Theme) -> Span<'static> {
    Span::styled(" · ", Style::default().fg(theme.muted))
}

pub fn rounded_block(theme: &Theme, focused: bool) -> Block<'static> {
    let border_color = if focused {
        theme.border_focus
    } else {
        theme.border
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
}
