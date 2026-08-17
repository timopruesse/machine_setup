use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::{
    details_mode, display_window, progress_label, selected_band_index, visible_runner_indices,
    DetailsMode,
};
use crate::tui::format::{format_duration, task_elapsed, task_palette_color};
use crate::tui::log_display;
use crate::tui::state::{TaskStatus, UiState};

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    match details_mode(state) {
        DetailsMode::RunnerGrid => render_runner_grid(f, area, state),
        DetailsMode::ExpandedTask => render_expanded(f, area, state),
        DetailsMode::SingleTask => render_single(f, area, state),
    }
}

fn render_runner_grid(f: &mut Frame, area: Rect, state: &UiState) {
    let mode = DetailsMode::RunnerGrid;
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
        let window = display_window(task, mode, body_h, scroll, !is_selected);

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
            .map(|line| log_line_to_ratatui(line))
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
    render_task_log(
        f,
        area,
        TaskLogPane {
            task,
            title: &title,
            log_scroll: state.log_scroll,
            border_color: Color::Yellow,
            mode: DetailsMode::ExpandedTask,
        },
    );
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
    render_task_log(
        f,
        area,
        TaskLogPane {
            task,
            title: &title,
            log_scroll: state.log_scroll,
            border_color,
            mode: DetailsMode::SingleTask,
        },
    );
}

struct TaskLogPane<'a> {
    task: &'a crate::tui::state::TaskState,
    title: &'a str,
    log_scroll: usize,
    border_color: Color,
    mode: DetailsMode,
}

fn render_task_log(f: &mut Frame, area: Rect, pane: TaskLogPane<'_>) {
    let TaskLogPane {
        task,
        title,
        log_scroll,
        border_color,
        mode,
    } = pane;
    let expanded = matches!(mode, DetailsMode::ExpandedTask);
    let mut status_spans = status_spans(task);

    let inner_height = area.height.saturating_sub(2) as usize;
    let display = log_display::display_lines(task, mode, expanded);
    let total_lines = display.len();

    let scroll = if total_lines > inner_height {
        let max_scroll = total_lines.saturating_sub(inner_height);
        log_scroll.min(max_scroll)
    } else {
        0
    };

    let lines: Vec<Line> = display
        .into_iter()
        .skip(scroll)
        .take(inner_height)
        .map(|line| log_line_to_ratatui(line))
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
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn log_line_to_ratatui(line: &crate::tui::state::LogLine) -> Line<'static> {
    use crate::engine::output::OutputKind;
    let mut style = log_display::style_for_kind(line.kind);
    if line.kind == OutputKind::TaskStatus {
        if line.text.starts_with("Failed") {
            style = style.fg(Color::Red);
        } else if line.text.starts_with("Completed") {
            style = style.fg(Color::Green);
        }
    }
    Line::from(Span::styled(line.text.clone(), style))
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
