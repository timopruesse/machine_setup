use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::format::{format_duration, task_elapsed, task_palette_color};
use crate::tui::state::{TaskStatus, UiState};

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    if state.in_merge_mode() {
        render_merge(f, area, state);
    } else {
        render_single(f, area, state);
    }
}

fn render_merge(f: &mut Frame, area: Rect, state: &UiState) {
    let n = state.running_count();
    let title = format!(" Parallel · {n} ");

    let inner_height = area.height.saturating_sub(2) as usize;
    let total_lines = state.merge_lines.len();
    let scroll = if total_lines > inner_height {
        let max_scroll = total_lines.saturating_sub(inner_height);
        state.log_scroll.min(max_scroll)
    } else {
        0
    };

    let name_budget = 16usize;
    let lines: Vec<Line> = state
        .merge_lines
        .iter()
        .map(|entry| {
            let color = task_palette_color(entry.color_idx);
            let name = truncate_name(&entry.task_name, name_budget);
            let text_style = line_style(&entry.text);
            Line::from(vec![
                Span::styled(
                    format!("{name} │ "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.text.as_str(), text_style),
            ])
        })
        .collect();

    let bottom = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{n} running"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · j/k list ", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))
                .title_bottom(bottom),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    f.render_widget(paragraph, area);
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

    let mut status_spans = match &task.status {
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
        TaskStatus::Failed(e) => {
            vec![Span::styled(
                format!("failed: {e}"),
                Style::default().fg(Color::Red),
            )]
        }
        TaskStatus::Skipped(r) => vec![Span::styled(
            format!("skipped: {r}"),
            Style::default().fg(Color::DarkGray),
        )],
    };

    if let Some(d) = task_elapsed(task.started_at, task.duration) {
        status_spans.push(Span::styled(
            format!(" · {}", format_duration(d)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if matches!(task.status, TaskStatus::Running) {
        if let Some(cmd) = task.current_command.as_deref() {
            let hint = truncate_cmd(cmd, 40);
            status_spans.push(Span::styled(
                format!(" · {hint}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let total_lines = task.log_lines.len();

    let scroll = if total_lines > inner_height {
        let max_scroll = total_lines.saturating_sub(inner_height);
        state.log_scroll.min(max_scroll)
    } else {
        0
    };

    let lines: Vec<Line> = task
        .log_lines
        .iter()
        .map(|line| Line::from(Span::styled(line.as_str(), line_style(line))))
        .collect();

    let border_color = match &task.status {
        TaskStatus::Running => Color::Yellow,
        TaskStatus::Completed => Color::Green,
        TaskStatus::Failed(_) => Color::Red,
        _ => Color::DarkGray,
    };

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

fn truncate_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        format!("{name:<max$}")
    } else {
        let take = max.saturating_sub(1);
        let mut out: String = name.chars().take(take).collect();
        out.push('…');
        out
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
