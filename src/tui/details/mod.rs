//! Details pane: view resolution and scroll helpers (pure, no ratatui).

mod render;

use super::state::{TaskState, UiState};

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

/// Log line count used for scroll in Runner grid mode (selected band only).
pub fn selected_band_log_len(state: &UiState) -> usize {
    let (visible, _) = visible_runner_indices(state);
    let idx = visible
        .get(selected_band_index(state))
        .copied()
        .or_else(|| visible.first().copied());
    idx.and_then(|i| state.tasks.get(i))
        .map(|t| t.log_lines.len())
        .unwrap_or(0)
}

/// Slice of log lines to show for a band in the grid.
pub fn band_log_window(
    task: &TaskState,
    inner_height: usize,
    scroll: usize,
    is_selected: bool,
) -> &[String] {
    let total = task.log_lines.len();
    if total == 0 || inner_height == 0 {
        return &[];
    }
    if is_selected {
        let max_scroll = total.saturating_sub(inner_height);
        let start = scroll.min(max_scroll);
        return &task.log_lines[start..start + inner_height.min(total - start)];
    }
    let start = total.saturating_sub(inner_height);
    &task.log_lines[start..]
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
    fn mode_expanded_when_flag_set() {
        let mut state = UiState::new(vec!["a".into(), "b".into()], Mode::Install);
        state.tasks[0].status = TaskStatus::Running;
        state.tasks[1].status = TaskStatus::Running;
        state.details_expanded = true;
        assert_eq!(details_mode(&state), DetailsMode::ExpandedTask);
    }

    #[test]
    fn visible_runners_capped_with_overflow() {
        let mut state = UiState::new((0..6).map(|i| format!("t{i}")).collect(), Mode::Install);
        for t in &mut state.tasks {
            t.status = TaskStatus::Running;
        }
        let (visible, overflow) = visible_runner_indices(&state);
        assert_eq!(visible.len(), MAX_VISIBLE_RUNNERS);
        assert_eq!(overflow, 2);
    }
}
