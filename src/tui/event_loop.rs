use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::engine::event::TaskEvent;

use super::message::{Effect, Input, Message};
use super::reduce::reduce;
use super::render;
use super::state::UiState;

/// Async UI loop: engine events + keyboard + spinner ticks.
/// Returns the final [`UiState`] for the post-run summary.
pub async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut state: UiState,
    mut event_rx: mpsc::Receiver<TaskEvent>,
    cancel: CancellationToken,
) -> anyhow::Result<UiState> {
    let (key_tx, mut key_rx) = mpsc::unbounded_channel::<KeyEvent>();
    let key_cancel = cancel.clone();
    std::thread::spawn(move || {
        while !key_cancel.is_cancelled() {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key_tx.send(key).is_err() {
                            break;
                        }
                    }
                }
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });

    let mut ticker = tokio::time::interval(Duration::from_millis(150));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // First tick completes immediately; skip it so we don't double-draw on entry.
    ticker.tick().await;

    terminal.draw(|f| render(f, &state))?;

    // Once the engine drops its sender, `recv()` stays Ready(None) forever. With
    // `biased` select that starves keyboard input — fuse the arm off after close.
    let mut events_open = true;

    loop {
        let mut dirty = false;

        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                break;
            }
            maybe_event = event_rx.recv(), if events_open => {
                match maybe_event {
                    Some(first) => {
                        state = apply_engine_batch(&mut event_rx, state, first);
                        dirty = true;
                    }
                    None => {
                        events_open = false;
                        if !state.done {
                            state.done = true;
                            state.freeze_run_elapsed();
                            dirty = true;
                        }
                    }
                }
            }
            maybe_key = key_rx.recv() => {
                let Some(key) = maybe_key else { break; };
                if let Some(input) = map_key(&state, key) {
                    let (next, effect) = reduce(state, Message::Input(input));
                    state = next;
                    dirty = true;
                    match effect {
                        Effect::None => {}
                        Effect::Quit => break,
                        Effect::CancelAndQuit => {
                            cancel.cancel();
                            break;
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                // Keep ticking until done so the run clock advances between tasks.
                if !state.done {
                    let (next, _) = reduce(state, Message::Tick);
                    state = next;
                    dirty = true;
                }
            }
        }

        if dirty {
            terminal.draw(|f| render(f, &state))?;
        }
    }

    Ok(state)
}

fn apply_engine_batch(
    event_rx: &mut mpsc::Receiver<TaskEvent>,
    mut state: UiState,
    first: TaskEvent,
) -> UiState {
    let (next, _) = reduce(state, Message::Engine(first));
    state = next;
    loop {
        match event_rx.try_recv() {
            Ok(ev) => {
                let (next, _) = reduce(state, Message::Engine(ev));
                state = next;
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                state.done = true;
                state.freeze_run_elapsed();
                break;
            }
        }
    }
    state
}

fn map_key(state: &UiState, key: KeyEvent) -> Option<Input> {
    // Ignore key release so one physical press == one input.
    if key.kind == KeyEventKind::Release {
        return None;
    }
    // Allow Repeat only for navigation keys (held j/k).
    if key.kind == KeyEventKind::Repeat
        && !matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
        )
    {
        return None;
    }

    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Input::CancelAndQuit);
    }

    if state.search_mode {
        return match key.code {
            KeyCode::Esc => Some(Input::ExitSearch),
            KeyCode::Enter => Some(Input::ConfirmSearch),
            KeyCode::Backspace => Some(Input::SearchBackspace),
            KeyCode::Up | KeyCode::Char('k') => Some(Input::SelectPrev),
            KeyCode::Down | KeyCode::Char('j') => Some(Input::SelectNext),
            KeyCode::Char(c) => Some(Input::SearchChar(c)),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Enter if state.in_burst_mode() && !state.search_mode => {
            Some(Input::ToggleDetailsExpand)
        }
        KeyCode::Char('q') => Some(Input::CancelAndQuit),
        KeyCode::Esc => Some(Input::ClearFilterOrQuit),
        KeyCode::Char('/') => Some(Input::EnterSearch),
        KeyCode::Up | KeyCode::Char('k') => Some(Input::SelectPrev),
        KeyCode::Down | KeyCode::Char('j') => Some(Input::SelectNext),
        KeyCode::PageUp => Some(Input::LogPageUp),
        KeyCode::PageDown => Some(Input::LogPageDown),
        KeyCode::Home => Some(Input::LogHome),
        KeyCode::End => Some(Input::LogEnd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mode::Mode;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn search_mode_jk_navigate_not_insert() {
        let mut state = UiState::new(vec!["a".into(), "b".into()], Mode::Install);
        state.search_mode = true;
        assert_eq!(
            map_key(&state, key(KeyCode::Char('j'))),
            Some(Input::SelectNext)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Char('k'))),
            Some(Input::SelectPrev)
        );
    }

    #[test]
    fn normal_mode_q_cancels_esc_clears_or_quits() {
        let state = UiState::new(vec!["a".into()], Mode::Install);
        assert_eq!(
            map_key(&state, key(KeyCode::Char('q'))),
            Some(Input::CancelAndQuit)
        );
        assert_eq!(
            map_key(&state, key(KeyCode::Esc)),
            Some(Input::ClearFilterOrQuit)
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let state = UiState::new(vec!["a".into()], Mode::Install);
        let mut release = key(KeyCode::Char('q'));
        release.kind = KeyEventKind::Release;
        assert_eq!(map_key(&state, release), None);
    }
}
