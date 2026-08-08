use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Gauge};
use ratatui::Frame;

use crate::tui::format::{format_duration, run_elapsed};
use crate::tui::state::UiState;

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    let completed = state.completed_tasks();
    let total = state.total_tasks();
    let ratio = if total > 0 {
        (completed as f64 / total as f64).min(1.0)
    } else {
        0.0
    };

    let elapsed = format_duration(run_elapsed(state.run_started, state.run_elapsed));

    let status = if state.done {
        if state.failed > 0 {
            format!(
                " Done: {} ok, {} failed, {} skipped  {elapsed} ",
                state.succeeded, state.failed, state.skipped
            )
        } else {
            format!(
                " Done: {} ok, {} skipped  {elapsed} ",
                state.succeeded, state.skipped
            )
        }
    } else {
        let running = state.running_count();
        if running >= 1 {
            format!(
                " {} {}/{}  {elapsed}  {running} running ",
                state.mode, completed, total
            )
        } else {
            format!(" {} {}/{}  {elapsed} ", state.mode, completed, total)
        }
    };

    let color = if state.done {
        if state.failed > 0 {
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
