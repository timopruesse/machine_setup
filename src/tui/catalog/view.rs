use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::theme::Theme;
use crate::tui::widgets::chrome::{hint_separator, key_hint, rounded_block};

use super::model::{CatalogItem, CatalogMode, CatalogStatus};
use super::state::CatalogState;

pub fn render(f: &mut Frame, state: &CatalogState, theme: &Theme) {
    let banner_height = state
        .banner
        .as_ref()
        .map(|lines| (lines.len().clamp(1, 6) as u16).saturating_add(2))
        .unwrap_or(0);

    let outer = if banner_height > 0 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(banner_height),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(f.area())
    };

    let (main_area, help_area) = if banner_height > 0 {
        render_banner(f, outer[0], state, theme);
        (outer[1], outer[2])
    } else {
        (outer[0], outer[1])
    };

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_area);

    render_list(f, main[0], state, theme);
    render_detail(f, main[1], state, theme);
    render_help(f, help_area, state, theme);
}

fn render_banner(f: &mut Frame, area: Rect, state: &CatalogState, theme: &Theme) {
    let Some(lines) = state.banner.as_ref() else {
        return;
    };
    let text: Vec<Line> = lines
        .iter()
        .take(6)
        .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(theme.text))))
        .collect();

    let paragraph = Paragraph::new(text)
        .block(rounded_block(theme, false).title(Span::styled(
            " Summary ",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_list(f: &mut Frame, area: Rect, state: &CatalogState, theme: &Theme) {
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
        .map(|(i, item)| ListItem::new(Line::from(list_row_spans(state, i, item, theme))))
        .collect();

    let selected_pos = state
        .filtered_indices
        .iter()
        .position(|&i| i == state.selected);

    let title = list_title(state);
    let list = List::new(items).block(rounded_block(theme, true).title(Span::styled(
        title,
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    )));

    let mut list_state = ListState::default();
    list_state.select(selected_pos);
    f.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(search_area) = search_area {
        render_search_line(f, search_area, state, theme);
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

fn list_row_spans(
    state: &CatalogState,
    index: usize,
    item: &CatalogItem,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let selected = index == state.selected;
    let indicator = if selected { ">" } else { " " };

    let mut spans = vec![Span::styled(
        format!("{indicator} "),
        if selected {
            Style::default()
                .fg(theme.accent)
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
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ));
    }

    let (glyph, glyph_style) = status_glyph(item, theme);
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
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        },
    ));

    if !item.badges.is_empty() {
        spans.push(Span::raw("  "));
        for (i, badge) in item.badges.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(badge.clone(), badge_style(badge, theme)));
        }
    }

    spans
}

fn badge_style(badge: &str, theme: &Theme) -> Style {
    match badge {
        "error" => Style::default().fg(theme.error),
        "warn" => Style::default().fg(theme.warning),
        _ => Style::default().fg(theme.muted),
    }
}

fn status_glyph(item: &CatalogItem, theme: &Theme) -> (&'static str, Style) {
    if item.badges.iter().any(|b| b == "error") {
        return ("!", Style::default().fg(theme.error));
    }
    match &item.status {
        CatalogStatus::Installed => ("✓", Style::default().fg(theme.success)),
        CatalogStatus::NotInstalled | CatalogStatus::Neutral => {
            ("·", Style::default().fg(theme.muted))
        }
        CatalogStatus::SkippedOs => ("–", Style::default().fg(theme.warning)),
    }
}

fn render_detail(f: &mut Frame, area: Rect, state: &CatalogState, theme: &Theme) {
    let lines = if state.filtered_indices.is_empty() {
        vec![Line::from(Span::styled(
            "No matches",
            Style::default().fg(theme.muted),
        ))]
    } else if let Some(item) = state.items.get(state.selected) {
        detail_lines(item, theme)
    } else {
        vec![Line::from(Span::styled(
            "(no selection)",
            Style::default().fg(theme.muted),
        ))]
    };

    let paragraph = Paragraph::new(lines)
        .block(rounded_block(theme, false).title(Span::styled(
            " Detail ",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn detail_lines(item: &CatalogItem, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for section in &item.detail {
        lines.push(Line::from(Span::styled(
            section.title.clone(),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )));
        for line in &section.lines {
            let style = if section.title == "Validation" {
                validation_line_style(line, theme)
            } else {
                Style::default().fg(theme.text)
            };
            lines.push(Line::from(Span::styled(line.clone(), style)));
        }
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no detail)",
            Style::default().fg(theme.muted),
        )));
    }
    lines
}

fn validation_line_style(line: &str, theme: &Theme) -> Style {
    if line.starts_with("[Error]") {
        Style::default().fg(theme.error)
    } else if line.starts_with("[Warning]") {
        Style::default().fg(theme.warning)
    } else {
        Style::default().fg(theme.text)
    }
}

fn render_search_line(f: &mut Frame, area: Rect, state: &CatalogState, theme: &Theme) {
    let search_line = Line::from(vec![
        Span::styled(
            "/",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&state.search_query, Style::default().fg(theme.text)),
        if state.search_mode {
            Span::styled("_", Style::default().fg(theme.accent))
        } else {
            Span::raw("")
        },
    ]);
    f.render_widget(Paragraph::new(search_line), area);
}

fn render_help(f: &mut Frame, area: Rect, state: &CatalogState, theme: &Theme) {
    let keys = if state.search_mode {
        vec![
            key_hint(theme, "Esc", "cancel"),
            key_hint(theme, "Enter", "apply"),
            key_hint(theme, "j/k", "navigate"),
        ]
    } else if state.filter_active() {
        let mut hints = vec![
            key_hint(theme, "Esc", "clear filter"),
            key_hint(
                theme,
                "q",
                if matches!(state.mode, CatalogMode::Browse) {
                    "quit"
                } else {
                    "abort"
                },
            ),
            key_hint(theme, "j/k", "navigate"),
            key_hint(theme, "/", "search"),
        ];
        if matches!(state.mode, CatalogMode::Select) {
            hints.push(key_hint(theme, "Space", "toggle"));
            hints.push(key_hint(theme, "a", "all visible"));
            hints.push(key_hint(theme, "Enter", "confirm"));
        }
        hints
    } else if matches!(state.mode, CatalogMode::Select) {
        vec![
            key_hint(theme, "q", "abort"),
            key_hint(theme, "j/k", "navigate"),
            key_hint(theme, "Space", "toggle"),
            key_hint(theme, "a", "all visible"),
            key_hint(theme, "Enter", "confirm"),
            key_hint(theme, "/", "search"),
        ]
    } else {
        vec![
            key_hint(theme, "q", "quit"),
            key_hint(theme, "j/k", "navigate"),
            key_hint(theme, "/", "search"),
        ]
    };

    let mut spans = Vec::new();
    for (i, group) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(hint_separator(theme));
        }
        spans.extend(group.clone());
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
