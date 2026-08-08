use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::format::{format_duration, run_elapsed, task_elapsed, task_palette_color};
use crate::tui::state::{TaskStatus, UiState};

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    let (list_area, search_area) = if state.filter_active() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let spinner = state.spinner_frame();
    // Approximate usable width for truncating the command hint.
    let hint_budget = list_area.width.saturating_sub(12) as usize;
    let running = state.running_count();

    let items: Vec<ListItem> = state
        .filtered_indices
        .iter()
        .filter_map(|&i| state.tasks.get(i).map(|task| (i, task)))
        .map(|(i, task)| {
            let accent = task
                .color_idx
                .map(task_palette_color)
                .unwrap_or(Color::Yellow);

            let (symbol, style) = match &task.status {
                TaskStatus::Pending => ("·", Style::default().fg(Color::DarkGray)),
                TaskStatus::Running => (
                    spinner,
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
                TaskStatus::Completed => ("✓", Style::default().fg(Color::Green)),
                TaskStatus::Failed(_) => (
                    "✗",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                TaskStatus::Skipped(_) => ("–", Style::default().fg(Color::DarkGray)),
            };

            let indicator = if i == state.selected { ">" } else { " " };
            let indent = "  ".repeat(task.depth);

            let mut spans = vec![
                Span::styled(
                    format!("{indicator} "),
                    if i == state.selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
                Span::raw(indent),
                Span::styled(format!("[{symbol}] "), style),
                Span::styled(
                    task.name.clone(),
                    if i == state.selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                ),
            ];

            if let Some(d) = task_elapsed(task.started_at, task.duration) {
                spans.push(Span::styled(
                    format!("  {}", format_duration(d)),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if matches!(task.status, TaskStatus::Running) {
                if let Some(cmd) = task.current_command.as_deref() {
                    let hint = truncate_hint(cmd, hint_budget.saturating_sub(task.name.len() + 8));
                    if !hint.is_empty() {
                        spans.push(Span::styled(
                            format!("  › {hint}"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let selected_pos = state
        .filtered_indices
        .iter()
        .position(|&i| i == state.selected);

    let total_time = format_duration(run_elapsed(state.run_started, state.run_elapsed));
    let mut bottom_spans = vec![
        Span::styled(" total ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            total_time,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if running >= 1 {
        bottom_spans.push(Span::styled(
            format!(" · {running} running "),
            Style::default().fg(Color::Yellow),
        ));
    } else {
        bottom_spans.push(Span::raw(" "));
    }
    let total_title = Line::from(bottom_spans).right_aligned();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Tasks ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_bottom(total_title),
    );

    let mut list_state = ListState::default();
    list_state.select(selected_pos);

    f.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(search_area) = search_area {
        let search_line = Line::from(vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.search_query, Style::default().fg(Color::White)),
            if state.search_mode {
                Span::styled("_", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]);
        let search = Paragraph::new(search_line);
        f.render_widget(search, search_area);
    }
}

fn truncate_hint(cmd: &str, max: usize) -> String {
    if max < 4 {
        return String::new();
    }
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
