use crate::engine::event::TaskEvent;

/// Side effects requested by [`crate::tui::reduce::reduce`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Leave the UI loop without cancelling the engine.
    Quit,
    /// Cancel the engine and leave the UI loop.
    CancelAndQuit,
}

/// Keyboard / UI intents (mapped from crossterm in the event loop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    CancelAndQuit,
    /// Esc: clear filter if active, otherwise quit.
    ClearFilterOrQuit,
    EnterSearch,
    ConfirmSearch,
    ExitSearch,
    SearchChar(char),
    SearchBackspace,
    SelectNext,
    SelectPrev,
    LogPageUp,
    LogPageDown,
    LogHome,
    LogEnd,
    /// Enter: expand/collapse full log during a parallel burst.
    ToggleDetailsExpand,
}

/// All messages the reducer accepts.
#[derive(Debug, Clone)]
pub enum Message {
    Engine(TaskEvent),
    Input(Input),
    Tick,
}
