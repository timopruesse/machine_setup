use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::tui::app::{App, TaskStatus};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .enumerate()
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
    state.select(Some(app.selected));

    f.render_stateful_widget(list, area, &mut state);
}
