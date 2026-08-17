use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::details;
use crate::tui::state::UiState;

pub fn render(f: &mut Frame, area: Rect, state: &UiState) {
    details::render(f, area, state);
}
