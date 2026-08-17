//! Details pane: view resolution and scroll helpers (pure, no ratatui).

mod render;

use super::log_display;
use super::state::{LogLine, TaskState, UiState};

pub use render::render;

/// Max running tasks shown as bands in the Runner grid.
pub const MAX_VISIBLE_RUNNERS: usize = 4;

/// Which presentation the details pane uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsMode {
    SingleTask,
    RunnerGrid,
    ExpandedTask,
}

pub fn details_mode(state: &UiState) -> DetailsMode {
    if state.in_burst_mode() {
        if state.details_expanded {
            DetailsMode::ExpandedTask
        } else {
            DetailsMode::RunnerGrid
        }
    } else {
        DetailsMode::SingleTask
    }
}

/// Visible running task indices (capped).
pub fn visible_runner_indices(state: &UiState) -> (Vec<usize>, usize) {
    let all = state.running_task_indices();
    let overflow = all.len().saturating_sub(MAX_VISIBLE_RUNNERS);
    let visible: Vec<usize> = all.into_iter().take(MAX_VISIBLE_RUNNERS).collect();
    (visible, overflow)
}

/// Index into the visible runner list for the selected task, if running and visible.
pub fn selected_band_index(state: &UiState) -> usize {
    let (visible, _) = visible_runner_indices(state);
    visible
        .iter()
        .position(|&i| i == state.selected)
        .unwrap_or(0)
}

pub fn task_display_len(task: &TaskState, mode: DetailsMode) -> usize {
    log_display::display_lines(task, mode, true).len()
}

/// Log line count used for scroll in Runner grid mode (selected band only).
pub fn selected_band_log_len(state: &UiState) -> usize {
    let mode = details_mode(state);
    let (visible, _) = visible_runner_indices(state);
    let idx = visible
        .get(selected_band_index(state))
        .copied()
        .or_else(|| visible.first().copied());
    idx.map(|i| {
        state
            .tasks
            .get(i)
            .map(|t| task_display_len(t, mode))
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Window of display lines for a task log or band body.
pub fn display_window(
    task: &TaskState,
    mode: DetailsMode,
    inner_height: usize,
    scroll: usize,
    tail: bool,
) -> Vec<&LogLine> {
    let lines = log_display::display_lines(task, mode, true);
    let total = lines.len();
    if total == 0 || inner_height == 0 {
        return Vec::new();
    }
    let start = if tail {
        total.saturating_sub(inner_height)
    } else {
        let max_scroll = total.saturating_sub(inner_height);
        scroll.min(max_scroll)
    };
    lines
        .into_iter()
        .skip(start)
        .take(inner_height.min(total.saturating_sub(start)))
        .collect()
}

pub fn progress_label(task: &TaskState) -> Option<String> {
    match (task.command_index, task.command_total) {
        (Some(i), Some(t)) if t > 0 => Some(format!("[{i}/{t}]")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mode::Mode;
    use crate::engine::output::OutputKind;
    use crate::tui::state::TaskStatus;

    #[test]
    fn mode_single_when_one_runner() {
        let state = UiState::new(vec!["a".into()], Mode::Install);
        assert_eq!(details_mode(&state), DetailsMode::SingleTask);
    }

    #[test]
    fn mode_grid_when_two_runners() {
        let mut state = UiState::new(vec!["a".into(), "b".into()], Mode::Install);
        state.tasks[0].status = TaskStatus::Running;
        state.tasks[1].status = TaskStatus::Running;
        assert_eq!(details_mode(&state), DetailsMode::RunnerGrid);
    }

    #[test]
    fn display_len_ignores_hidden_done() {
        let mut task = TaskState::new("a".into());
        task.push_log(OutputKind::Progress, "x".into());
        task.push_log(OutputKind::CommandDone, "y".into());
        assert_eq!(task_display_len(&task, DetailsMode::SingleTask), 1);
    }
}
