use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::{App, TaskStatus};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let task = match app.selected_task() {
        Some(t) => t,
        None => {
            let empty = Paragraph::new("No tasks")
                .block(Block::default().borders(Borders::ALL).title(" Log "));
            f.render_widget(empty, area);
            return;
        }
    };

    let title = format!(" {} ", task.name);

    let status_info = match &task.status {
        TaskStatus::Pending => Span::styled("pending", Style::default().fg(Color::DarkGray)),
        TaskStatus::Running => Span::styled(
            "running",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        TaskStatus::Completed => Span::styled("completed", Style::default().fg(Color::Green)),
        TaskStatus::Failed(e) => Span::styled(
            format!("failed: {e}"),
            Style::default().fg(Color::Red),
        ),
        TaskStatus::Skipped(r) => Span::styled(
            format!("skipped: {r}"),
            Style::default().fg(Color::DarkGray),
        ),
    };

    // Build log lines with syntax highlighting
    let inner_height = area.height.saturating_sub(2) as usize; // borders
    let total_lines = task.log_lines.len();

    // Compute scroll offset to show the end of logs
    let scroll = if total_lines > inner_height {
        // If auto-scrolling, show the latest lines
        let max_scroll = total_lines.saturating_sub(inner_height);
        app.log_scroll.min(max_scroll)
    } else {
        0
    };

    let lines: Vec<Line> = task
        .log_lines
        .iter()
        .map(|line| {
            let style = if line.starts_with("> ") {
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
            };
            Line::from(Span::styled(line.as_str(), style))
        })
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
                .title_bottom(Line::from(vec![
                    Span::raw(" "),
                    status_info,
                    Span::raw(" "),
                ])),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    f.render_widget(paragraph, area);
}
