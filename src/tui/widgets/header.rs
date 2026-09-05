use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Gauge;
use ratatui::Frame;

use crate::tui::format::{format_duration, run_elapsed};
use crate::tui::state::UiState;
use crate::tui::theme::Theme;
use crate::tui::widgets::chrome::rounded_block;

pub fn render(f: &mut Frame, area: Rect, state: &UiState, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

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
                " {} ok · {} failed · {elapsed} ",
                state.succeeded, state.failed
            )
        } else if state.skipped > 0 {
            format!(
                " {} ok · {} skipped · {elapsed} ",
                state.succeeded, state.skipped
            )
        } else {
            format!(" {} ok · {elapsed} ", state.succeeded)
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

    let (border, fill) = if state.done {
        if state.failed > 0 {
            (theme.error, theme.gauge_fill_err)
        } else {
            (theme.success, theme.gauge_fill_ok)
        }
    } else {
        (theme.accent_alt, theme.gauge_fill_run)
    };

    let gauge = Gauge::default()
        .block(
            rounded_block(theme, false)
                .border_style(Style::default().fg(border))
                .title(Span::styled(
                    " machine_setup ",
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                )),
        )
        .gauge_style(Style::default().fg(fill).bg(theme.gauge_bg))
        .ratio(ratio)
        .label(Span::styled(
            status,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ));

    f.render_widget(gauge, area);
}
