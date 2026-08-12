use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::model::{CatalogItem, CatalogMode, CatalogStatus};
use super::state::CatalogState;

pub fn render(f: &mut Frame, state: &CatalogState) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(outer[0]);

    render_list(f, main[0], state);
    render_detail(f, main[1], state);
    render_help(f, outer[1], state);
}

fn render_list(f: &mut Frame, area: Rect, state: &CatalogState) {
    let (list_area, search_area) = if state.filter_active() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let items: Vec<ListItem> = state
        .filtered_indices
        .iter()
        .filter_map(|&i| state.items.get(i).map(|item| (i, item)))
        .map(|(i, item)| ListItem::new(Line::from(list_row_spans(state, i, item))))
        .collect();

    let selected_pos = state
        .filtered_indices
        .iter()
        .position(|&i| i == state.selected);

    let title = list_title(state);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    let mut list_state = ListState::default();
    list_state.select(selected_pos);
    f.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(search_area) = search_area {
        render_search_line(f, search_area, state);
    }
}

fn list_title(state: &CatalogState) -> String {
    let total = state.items.len();
    if state.filter_active() && state.filtered_indices.len() != total {
        format!(" Tasks ({}/{}) ", state.filtered_indices.len(), total)
    } else {
        format!(" Tasks ({total}) ")
    }
}

fn list_row_spans(state: &CatalogState, index: usize, item: &CatalogItem) -> Vec<Span<'static>> {
    let selected = index == state.selected;
    let indicator = if selected { ">" } else { " " };

    let mut spans = vec![Span::styled(
        format!("{indicator} "),
        if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        },
    )];

    if matches!(state.mode, CatalogMode::Select) {
        let checked = state.checked.contains(&index);
        let mark = if checked { "x" } else { " " };
        spans.push(Span::styled(
            format!("[{mark}] "),
            if checked {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));
    }

    let (glyph, glyph_style) = status_glyph(&item.status);
    spans.push(Span::styled(
        format!("[{glyph}] "),
        glyph_style.add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }),
    ));

    spans.push(Span::styled(
        item.title.clone(),
        if selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        },
    ));

    if !item.badges.is_empty() {
        spans.push(Span::styled(
            format!("  {}", item.badges.join(" ")),
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans
}

fn status_glyph(status: &CatalogStatus) -> (&'static str, Style) {
    match status {
        CatalogStatus::Installed => ("✓", Style::default().fg(Color::Green)),
        CatalogStatus::NotInstalled | CatalogStatus::Neutral => {
            ("·", Style::default().fg(Color::DarkGray))
        }
        CatalogStatus::SkippedOs => ("–", Style::default().fg(Color::Yellow)),
    }
}

fn render_detail(f: &mut Frame, area: Rect, state: &CatalogState) {
    let lines = if state.filtered_indices.is_empty() {
        vec![Line::from(Span::styled(
            "No matches",
            Style::default().fg(Color::DarkGray),
        ))]
    } else if let Some(item) = state.items.get(state.selected) {
        detail_lines(item)
    } else {
        vec![Line::from(Span::styled(
            "(no selection)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " Detail ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn detail_lines(item: &CatalogItem) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for section in &item.detail {
        lines.push(Line::from(Span::styled(
            section.title.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        for line in &section.lines {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(Color::White),
            )));
        }
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no detail)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn render_search_line(f: &mut Frame, area: Rect, state: &CatalogState) {
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
    f.render_widget(Paragraph::new(search_line), area);
}

fn render_help(f: &mut Frame, area: Rect, state: &CatalogState) {
    let keys = if state.search_mode {
        vec![
            key_hint("Esc", "cancel"),
            key_hint("Enter", "apply"),
            key_hint("j/k", "navigate"),
        ]
    } else if state.filter_active() {
        let mut hints = vec![
            key_hint("Esc", "clear filter"),
            key_hint(
                "q",
                if matches!(state.mode, CatalogMode::Browse) {
                    "quit"
                } else {
                    "abort"
                },
            ),
            key_hint("j/k", "navigate"),
            key_hint("/", "search"),
        ];
        if matches!(state.mode, CatalogMode::Select) {
            hints.push(key_hint("Space", "toggle"));
            hints.push(key_hint("a", "all visible"));
            hints.push(key_hint("Enter", "confirm"));
        }
        hints
    } else if matches!(state.mode, CatalogMode::Select) {
        vec![
            key_hint("q", "abort"),
            key_hint("j/k", "navigate"),
            key_hint("Space", "toggle"),
            key_hint("a", "all visible"),
            key_hint("Enter", "confirm"),
            key_hint("/", "search"),
        ]
    } else {
        vec![
            key_hint("q", "quit"),
            key_hint("j/k", "navigate"),
            key_hint("/", "search"),
        ]
    };

    let mut spans = Vec::new();
    for (i, group) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
        }
        spans.extend(group.clone());
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn key_hint(key: &str, action: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {action}"), Style::default().fg(Color::DarkGray)),
    ]
}
