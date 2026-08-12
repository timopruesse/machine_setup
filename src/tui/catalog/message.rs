/// Side effects requested by [`super::reduce::reduce`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogEffect {
    None,
    Quit,
    Abort,
    Confirm(Vec<String>),
}

/// Keyboard / UI intents (mapped from crossterm in the event loop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogInput {
    Quit,
    Abort,
    /// Esc: clear filter if active, otherwise leave.
    ClearFilterOrLeave,
    EnterSearch,
    ConfirmSearch,
    ExitSearch,
    SearchChar(char),
    SearchBackspace,
    SelectNext,
    SelectPrev,
    ToggleCheck,
    SelectAllVisible,
    ConfirmSelection,
}

/// All messages the catalog reducer accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogMessage {
    Input(CatalogInput),
}
