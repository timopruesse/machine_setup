use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

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

    let items: Vec<ListItem> = state
        .filtered_indices
        .iter()
        .filter_map(|&i| state.tasks.get(i).map(|task| (i, task)))
        .map(|(i, task)| {
            let (symbol, style) = match &task.status {
                TaskStatus::Pending => ("  ", Style::default().fg(Color::DarkGray)),
                TaskStatus::Running => (
                    spinner,
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

            let indicator = if i == state.selected { ">" } else { " " };
            let indent = "  ".repeat(task.depth);

            let line = Line::from(vec![
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
                    &task.name,
                    if i == state.selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let selected_pos = state
        .filtered_indices
        .iter()
        .position(|&i| i == state.selected);

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
