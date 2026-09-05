use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::UiState;
use crate::tui::theme::Theme;
use crate::tui::widgets::chrome::{hint_separator, key_hint};

pub fn render(f: &mut Frame, area: Rect, state: &UiState, theme: &Theme) {
    let hints = help_hints(state);
    let mut spans = Vec::new();
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(hint_separator(theme));
        }
        spans.extend(key_hint(theme, key, action));
    }

    let help = Paragraph::new(Line::from(spans));
    f.render_widget(help, area);
}

pub(crate) fn help_hints(state: &UiState) -> Vec<(&'static str, &'static str)> {
    let mut hints = if state.search_mode {
        vec![("Esc", "cancel"), ("Enter", "apply"), ("j/k", "navigate")]
    } else if state.filter_active() {
        let q_action = if state.done { "quit" } else { "cancel" };
        vec![
            ("Esc", "clear filter"),
            ("q", q_action),
            ("j/k", "navigate"),
            ("/", "search"),
        ]
    } else if state.done {
        vec![
            ("Esc", "quit"),
            ("q", "quit"),
            ("j/k", "navigate"),
            ("/", "search"),
        ]
    } else {
        let mut running = vec![("q", "cancel")];
        if state.in_burst_mode() {
            running.push(if state.details_expanded {
                ("Enter", "collapse")
            } else {
                ("Enter", "expand")
            });
            running.push(if state.details_expanded {
                ("j/k", "navigate")
            } else {
                ("j/k", "band")
            });
        } else {
            running.push(("j/k", "navigate"));
        }
        running.extend([
            ("PgUp/PgDn", "scroll"),
            ("Home/End", "log"),
            ("/", "search"),
        ]);
        running
    };

    if !state.log_follow && !state.search_mode {
        hints.push(("End", "follow"));
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mode::Mode;

    #[test]
    fn done_idle_advertises_esc_quit() {
        let mut state = UiState::new(vec!["a".into()], Mode::Install);
        state.done = true;
        let hints = help_hints(&state);
        assert!(hints.iter().any(|(k, a)| *k == "Esc" && *a == "quit"));
    }

    #[test]
    fn running_advertises_q_cancel() {
        let state = UiState::new(vec!["a".into()], Mode::Install);
        let hints = help_hints(&state);
        assert!(hints.iter().any(|(k, a)| *k == "q" && *a == "cancel"));
    }

    #[test]
    fn filter_active_while_running_advertises_q_cancel() {
        let mut state = UiState::new(vec!["a".into()], Mode::Install);
        state.search_query = "foo".into();
        let hints = help_hints(&state);
        assert!(hints.iter().any(|(k, a)| *k == "q" && *a == "cancel"));
    }

    #[test]
    fn filter_active_when_done_advertises_q_quit() {
        let mut state = UiState::new(vec!["a".into()], Mode::Install);
        state.done = true;
        state.search_query = "foo".into();
        let hints = help_hints(&state);
        assert!(hints.iter().any(|(k, a)| *k == "q" && *a == "quit"));
    }

    #[test]
    fn unfollowed_log_appends_end_follow() {
        let mut state = UiState::new(vec!["a".into()], Mode::Install);
        state.log_follow = false;
        let hints = help_hints(&state);
        assert!(hints.iter().any(|(k, a)| *k == "End" && *a == "follow"));
    }

    #[test]
    fn search_mode_does_not_append_end_follow() {
        let mut state = UiState::new(vec!["a".into()], Mode::Install);
        state.search_mode = true;
        state.log_follow = false;
        let hints = help_hints(&state);
        assert!(!hints.iter().any(|(k, _)| *k == "End"));
    }
}
