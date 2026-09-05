use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::details;
use crate::tui::state::UiState;
use crate::tui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &UiState, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    details::render(f, area, state, theme);
}
