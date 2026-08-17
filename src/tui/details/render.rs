use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::{
    band_log_window, details_mode, progress_label, selected_band_index, visible_runner_indices,
    DetailsMode,
};
use crate::tui::format::{format_duration, task_elapsed, task_palette_color};
use crate::tui::state::{TaskStatus, UiState};

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    match details_mode(state) {
        DetailsMode::RunnerGrid => render_runner_grid(f, area, state),
        DetailsMode::ExpandedTask => render_expanded(f, area, state),
        DetailsMode::SingleTask => render_single(f, area, state),
    }
}

fn render_runner_grid(f: &mut Frame, area: Rect, state: &UiState) {
    let (visible, overflow) = visible_runner_indices(state);
    let n = visible.len();
    if n == 0 {
        render_single(f, area, state);
        return;
    }

    let running = state.running_count();
    let mut title = format!(" Parallel · {running} ");
    if overflow > 0 {
        title.push_str(&format!("(+{overflow} more) "));
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(vec![
            Span::raw(" "),
            Span::styled("Enter expand", Style::default().fg(Color::DarkGray)),
            Span::styled(" · j/k band ", Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
        ]));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let band_heights: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n as u32)).collect();
    let bands = Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_heights)
        .split(inner);

    let selected_band = selected_band_index(state);

    for (band_i, (&task_idx, band_area)) in visible.iter().zip(bands.iter()).enumerate() {
        let task = &state.tasks[task_idx];
        let is_selected = band_i == selected_band;
        let accent = task
            .color_idx
            .map(task_palette_color)
            .unwrap_or(Color::Yellow);

        let inner_h = band_area.height.saturating_sub(2) as usize;
        let body_h = inner_h.saturating_sub(1).max(1);
        let scroll = if is_selected { state.log_scroll } else { 0 };
        let window = band_log_window(task, body_h, scroll, is_selected);

        let mut header = vec![Span::styled(
            task.name.as_str(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )];
        if let Some(label) = progress_label(task) {
            header.push(Span::styled(
                format!(" {label}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if let Some(d) = task_elapsed(task.started_at, task.duration) {
            header.push(Span::styled(
                format!(" · {}", format_duration(d)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if let Some(cmd) = task.current_command.as_deref() {
            header.push(Span::styled(
                format!(" · {}", truncate_cmd(cmd, 32)),
                Style::default().fg(Color::DarkGray),
            ));
        }

        let body: Vec<Line> = window
            .iter()
            .map(|line| Line::from(Span::styled(line.as_str(), line_style(line))))
            .collect();

        let border = if is_selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(Line::from(header));

        let paragraph = Paragraph::new(body).block(block).wrap(Wrap { trim: false });

        f.render_widget(paragraph, *band_area);
    }
}

fn render_expanded(f: &mut Frame, area: Rect, state: &UiState) {
    let others = state.running_count().saturating_sub(1);
    let task = match state.selected_task() {
        Some(t) => t,
        None => {
            render_single(f, area, state);
            return;
        }
    };

    let title = format!(
        " {} · expanded ({others} other{} running) ",
        task.name,
        if others == 1 { "" } else { "s" }
    );
    render_task_log(f, area, task, &title, state.log_scroll, Color::Yellow, true);
}

fn render_single(f: &mut Frame, area: Rect, state: &UiState) {
    let task = match state.selected_task() {
        Some(t) => t,
        None => {
            let empty = Paragraph::new("No tasks")
                .block(Block::default().borders(Borders::ALL).title(" Log "));
            f.render_widget(empty, area);
            return;
        }
    };

    let title = format!(" {} ", task.name);
    let border_color = status_border_color(&task.status);
    render_task_log(f, area, task, &title, state.log_scroll, border_color, false);
}

fn render_task_log(
    f: &mut Frame,
    area: Rect,
    task: &crate::tui::state::TaskState,
    title: &str,
    log_scroll: usize,
    border_color: Color,
    expanded: bool,
) {
    let mut status_spans = status_spans(task);

    let inner_height = area.height.saturating_sub(2) as usize;
    let total_lines = task.log_lines.len();

    let scroll = if total_lines > inner_height {
        let max_scroll = total_lines.saturating_sub(inner_height);
        log_scroll.min(max_scroll)
    } else {
        0
    };

    let lines: Vec<Line> = task
        .log_lines
        .iter()
        .map(|line| Line::from(Span::styled(line.as_str(), line_style(line))))
        .collect();

    if expanded {
        status_spans.push(Span::styled(
            " · Enter collapse",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))
                .title_bottom(Line::from({
                    let mut bottom = vec![Span::raw(" ")];
                    bottom.extend(status_spans);
                    bottom.push(Span::raw(" "));
                    bottom
                })),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    f.render_widget(paragraph, area);
}

fn status_spans(task: &crate::tui::state::TaskState) -> Vec<Span<'static>> {
    let mut spans = match &task.status {
        TaskStatus::Pending => vec![Span::styled(
            "pending",
            Style::default().fg(Color::DarkGray),
        )],
        TaskStatus::Running => vec![Span::styled(
            "running",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )],
        TaskStatus::Completed => vec![Span::styled("completed", Style::default().fg(Color::Green))],
        TaskStatus::Failed(e) => vec![Span::styled(
            format!("failed: {e}"),
            Style::default().fg(Color::Red),
        )],
        TaskStatus::Skipped(r) => vec![Span::styled(
            format!("skipped: {r}"),
            Style::default().fg(Color::DarkGray),
        )],
    };

    if let Some(label) = progress_label(task) {
        spans.push(Span::styled(
            format!(" · {label}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if let Some(d) = task_elapsed(task.started_at, task.duration) {
        spans.push(Span::styled(
            format!(" · {}", format_duration(d)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if matches!(task.status, TaskStatus::Running) {
        if let Some(cmd) = task.current_command.as_deref() {
            spans.push(Span::styled(
                format!(" · {}", truncate_cmd(cmd, 40)),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    spans
}

fn status_border_color(status: &TaskStatus) -> Color {
    match status {
        TaskStatus::Running => Color::Yellow,
        TaskStatus::Completed => Color::Green,
        TaskStatus::Failed(_) => Color::Red,
        _ => Color::DarkGray,
    }
}

fn line_style(line: &str) -> Style {
    if line.starts_with("> ") {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if line.contains("[FAILED]") {
        Style::default().fg(Color::Red)
    } else if line.contains("[done]") {
        Style::default().fg(Color::Green)
    } else if line.starts_with("  [stderr]") {
        Style::default().fg(Color::Yellow)
    } else if line.starts_with("Completed") {
        Style::default().fg(Color::Green)
    } else if line.starts_with("FAILED") {
        Style::default().fg(Color::Red)
    } else if line.starts_with("Skipped") {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    }
}

fn truncate_cmd(cmd: &str, max: usize) -> String {
    let trimmed = cmd.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let take = max.saturating_sub(1);
        let mut out: String = trimmed.chars().take(take).collect();
        out.push('…');
        out
    }
}
