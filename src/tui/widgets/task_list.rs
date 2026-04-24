use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, TaskStatus};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // Split area: main task list + optional search bar at bottom
    let (list_area, search_area) = if app.search_mode || !app.search_query.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // `filtered_indices` is already the exact set we want to render and is
    // maintained in ascending order by `update_filter`, so we can iterate it
    // directly without rebuilding a HashSet every frame.
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .filter_map(|&i| app.tasks.get(i).map(|task| (i, task)))
        .map(|(i, task)| {
            let (symbol, style) = match &task.status {
                TaskStatus::Pending => ("  ", Style::default().fg(Color::DarkGray)),
                TaskStatus::Running => (
                    ">>",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                TaskStatus::Completed => ("OK", Style::default().fg(Color::Green)),
                TaskStatus::Failed(_) => (
                    "XX",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                TaskStatus::Skipped(_) => ("--", Style::default().fg(Color::DarkGray)),
            };

            let indicator = if i == app.selected { ">" } else { " " };
            let indent = "  ".repeat(task.depth);

            let line = Line::from(vec![
                Span::styled(
                    format!("{indicator} "),
                    if i == app.selected {
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
                    &task.name,
                    if i == app.selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Compute which filtered position is selected
    let selected_pos = app.filtered_indices.iter().position(|&i| i == app.selected);

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Tasks ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    let mut state = ListState::default();
    state.select(selected_pos);

    f.render_stateful_widget(list, list_area, &mut state);

    // Render search bar if active or has a query
    if let Some(search_area) = search_area {
        let search_line = Line::from(vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.search_query, Style::default().fg(Color::White)),
            if app.search_mode {
                Span::styled("_", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]);
        let search = Paragraph::new(search_line);
        f.render_widget(search, search_area);
    }
}
