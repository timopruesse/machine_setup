use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Gauge};
use ratatui::Frame;

use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let completed = app.completed_tasks();
    let total = app.total_tasks();
    let ratio = if total > 0 {
        (completed as f64 / total as f64).min(1.0)
    } else {
        0.0
    };

    let status = if app.done {
        if app.failed > 0 {
            format!(
                " Done: {} ok, {} failed, {} skipped ",
                app.succeeded, app.failed, app.skipped
            )
        } else {
            format!(" Done: {} ok, {} skipped ", app.succeeded, app.skipped)
        }
    } else {
        format!(" {} {}/{} ", app.mode, completed, total)
    };

    let color = if app.done {
        if app.failed > 0 {
            Color::Red
        } else {
            Color::Green
        }
    } else {
        Color::Cyan
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
                .title(Span::styled(
                    " machine_setup ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(Span::styled(
            status,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(gauge, area);
}
