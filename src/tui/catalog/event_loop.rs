use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use super::message::{CatalogEffect, CatalogInput, CatalogMessage};
use super::model::CatalogMode;
use super::reduce::reduce;
use super::state::CatalogState;
use super::view;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopOutcome {
    Quit,
    Abort,
    Confirm(Vec<String>),
}

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut state: CatalogState,
) -> anyhow::Result<LoopOutcome> {
    terminal.draw(|f| view::render(f, &state))?;

    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let Some(input) = map_key(&state, key) {
                    let (next, effect) = reduce(state, CatalogMessage::Input(input));
                    state = next;
                    terminal.draw(|f| view::render(f, &state))?;
                    match effect {
                        CatalogEffect::None => {}
                        CatalogEffect::Quit => return Ok(LoopOutcome::Quit),
                        CatalogEffect::Abort => return Ok(LoopOutcome::Abort),
                        CatalogEffect::Confirm(ids) => return Ok(LoopOutcome::Confirm(ids)),
                    }
                }
            }
        }
    }
}

fn map_key(state: &CatalogState, key: KeyEvent) -> Option<CatalogInput> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.kind == KeyEventKind::Repeat
        && !matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
        )
    {
        return None;
    }

    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(CatalogInput::Quit);
    }

    if state.search_mode {
        return match key.code {
            KeyCode::Esc => Some(CatalogInput::ExitSearch),
            KeyCode::Enter => Some(CatalogInput::ConfirmSearch),
            KeyCode::Backspace => Some(CatalogInput::SearchBackspace),
            KeyCode::Up | KeyCode::Char('k') => Some(CatalogInput::SelectPrev),
            KeyCode::Down | KeyCode::Char('j') => Some(CatalogInput::SelectNext),
            KeyCode::Char(c) => Some(CatalogInput::SearchChar(c)),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Some(CatalogInput::Quit),
        KeyCode::Esc => Some(CatalogInput::ClearFilterOrLeave),
        KeyCode::Char('/') => Some(CatalogInput::EnterSearch),
        KeyCode::Up | KeyCode::Char('k') => Some(CatalogInput::SelectPrev),
        KeyCode::Down | KeyCode::Char('j') => Some(CatalogInput::SelectNext),
        KeyCode::Char(' ') if matches!(state.mode, CatalogMode::Select) => {
            Some(CatalogInput::ToggleCheck)
        }
        KeyCode::Char('a') if matches!(state.mode, CatalogMode::Select) => {
            Some(CatalogInput::SelectAllVisible)
        }
        KeyCode::Enter if matches!(state.mode, CatalogMode::Select) => {
            Some(CatalogInput::ConfirmSelection)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::catalog::model::{CatalogItem, CatalogMode, CatalogStatus};

    fn browse_state() -> CatalogState {
        CatalogState::new(
            vec![CatalogItem {
                id: "a".into(),
                title: "a".into(),
                status: CatalogStatus::NotInstalled,
                os_label: "all".into(),
                installed_at: "-".into(),
                updated_at: "-".into(),
                badges: vec![],
                detail: vec![],
            }],
            CatalogMode::Browse,
        )
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn search_mode_jk_navigate_not_insert() {
        let mut state = browse_state();
        state.search_mode = true;
        assert_eq!(
            map_key(&state, key(KeyCode::Char('j'))),
            Some(CatalogInput::SelectNext)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Char('k'))),
            Some(CatalogInput::SelectPrev)
        );
    }

    #[test]
    fn normal_mode_q_quits_esc_clears_or_leaves() {
        let state = browse_state();
        assert_eq!(
            map_key(&state, key(KeyCode::Char('q'))),
            Some(CatalogInput::Quit)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Esc)),
            Some(CatalogInput::ClearFilterOrLeave)
        );
    }

    #[test]
    fn browse_mode_space_returns_none() {
        let state = browse_state();
        assert_eq!(map_key(&state, key(KeyCode::Char(' '))), None);
    }

    #[test]
    fn select_mode_space_and_enter_map() {
        let state = CatalogState::new(
            vec![CatalogItem {
                id: "a".into(),
                title: "a".into(),
                status: CatalogStatus::NotInstalled,
                os_label: "all".into(),
                installed_at: "-".into(),
                updated_at: "-".into(),
                badges: vec![],
                detail: vec![],
            }],
            CatalogMode::Select,
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Char(' '))),
            Some(CatalogInput::ToggleCheck)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Enter)),
            Some(CatalogInput::ConfirmSelection)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Char('a'))),
            Some(CatalogInput::SelectAllVisible)
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let state = browse_state();
        let mut release = key(KeyCode::Char('q'));
        release.kind = KeyEventKind::Release;
        assert_eq!(map_key(&state, release), None);
    }

    #[test]
    fn ctrl_c_maps_to_quit() {
        let state = browse_state();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(&state, ctrl_c), Some(CatalogInput::Quit));
    }

    #[test]
    fn select_mode_ctrl_c_maps_to_quit() {
        let state = CatalogState::new(
            vec![CatalogItem {
                id: "a".into(),
                title: "a".into(),
                status: CatalogStatus::NotInstalled,
                os_label: "all".into(),
                installed_at: "-".into(),
                updated_at: "-".into(),
                badges: vec![],
                detail: vec![],
            }],
            CatalogMode::Select,
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(&state, ctrl_c), Some(CatalogInput::Quit));
    }
}
