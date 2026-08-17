//! Parallel burst presentation: selection and failure tracking while ≥2 Tasks run.

use super::state::{TaskState, UiState};

/// Hint for soft auto-select after an engine event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftSelect {
    None,
    Prefer(String),
    AnyRunning,
}

/// Context passed after each engine event mutates task state.
#[derive(Debug, Clone)]
pub struct BurstContext {
    pub running_before: usize,
    pub running_after: usize,
    pub task_name: String,
    pub soft: SoftSelect,
    pub task_failed_during_burst: bool,
}

/// Apply parallel-burst selection rules after task state is updated.
pub fn after_engine_event(state: &mut UiState, ctx: BurstContext) {
    if ctx.task_failed_during_burst {
        if let Some(idx) = state.tasks.iter().position(|t| t.name == ctx.task_name) {
            if !state.burst_failed.contains(&idx) {
                state.burst_failed.push(idx);
            }
        }
    }

    if ctx.running_before >= 2 && ctx.running_after <= 1 {
        leave_burst(state);
    } else if ctx.running_after < 2 {
        state.details_expanded = false;
        state.burst_failed.clear();
        apply_soft_select(state, ctx.soft);
    } else if !state.details_expanded {
        apply_soft_select(state, ctx.soft);
    }
    sync_log_scroll(state);
}

fn leave_burst(state: &mut UiState) {
    state.details_expanded = false;
    if let Some(&idx) = state.burst_failed.first() {
        state.selected = idx;
        state.log_follow = false;
        if let Some(task) = state.tasks.get(idx) {
            state.log_scroll = task.log_lines.len().saturating_sub(1);
        }
    } else if state.auto_select_running {
        if let Some(idx) = state.tasks.iter().position(|t| t.status.is_running()) {
            state.selected = idx;
            state.log_scroll = 0;
            state.log_follow = true;
        }
    }
    state.burst_failed.clear();
}

fn apply_soft_select(state: &mut UiState, soft: SoftSelect) {
    match soft {
        SoftSelect::None => {}
        SoftSelect::Prefer(name) => soft_auto_select(state, Some(name.as_str())),
        SoftSelect::AnyRunning => soft_auto_select(state, None),
    }
}

fn soft_auto_select(state: &mut UiState, preferred: Option<&str>) {
    if !state.auto_select_running {
        return;
    }
    let selected_running = state
        .tasks
        .get(state.selected)
        .map(|t| t.status.is_running())
        .unwrap_or(false);
    if selected_running {
        return;
    }
    if let Some(name) = preferred {
        select_task(state, name);
        return;
    }
    if let Some(idx) = state.tasks.iter().position(|t| t.status.is_running()) {
        state.selected = idx;
        state.log_scroll = 0;
        state.log_follow = true;
    }
}

pub fn select_task(state: &mut UiState, name: &str) {
    if let Some(idx) = state.tasks.iter().position(|t| t.name == name) {
        state.selected = idx;
        state.log_scroll = 0;
        state.log_follow = true;
    }
}

/// Keep scroll pinned to the end when follow mode is on.
pub fn sync_log_scroll(state: &mut UiState) {
    if state.log_follow {
        stick_log_to_end(state);
    }
}

pub fn active_log_len(state: &UiState) -> usize {
    use super::details;

    let mode = details::details_mode(state);
    if state.in_burst_mode() && !state.details_expanded {
        details::selected_band_log_len(state)
    } else {
        state
            .tasks
            .get(state.selected)
            .map(|t| details::task_display_len(t, mode))
            .unwrap_or(0)
    }
}

pub fn stick_log_to_end(state: &mut UiState) {
    let line_count = active_log_len(state);
    if line_count > 0 {
        state.log_scroll = line_count.saturating_sub(1);
    }
}

pub fn maybe_reenable_follow_at_bottom(state: &mut UiState) {
    let line_count = active_log_len(state);
    if line_count == 0 {
        return;
    }
    let end = line_count.saturating_sub(1);
    if state.log_scroll >= end {
        state.log_follow = true;
        state.log_scroll = end;
    }
}

pub fn find_or_create_task<'a>(state: &'a mut UiState, name: &str) -> &'a mut TaskState {
    if let Some(idx) = state.tasks.iter().position(|t| t.name == name) {
        return &mut state.tasks[idx];
    }
    let idx = state.tasks.len();
    state.tasks.push(TaskState::new(name.to_string()));
    let query = state.search_query.to_lowercase();
    if query.is_empty() || name.to_lowercase().contains(&query) {
        state.filtered_indices.push(idx);
    }
    &mut state.tasks[idx]
}

pub fn set_command_progress(
    task: &mut TaskState,
    command_index: usize,
    command_total: usize,
    command_desc: &str,
) {
    task.command_index = Some(command_index);
    task.command_total = Some(command_total);
    task.current_command = Some(command_desc.to_string());
}

pub fn clear_command_progress(task: &mut TaskState) {
    task.current_command = None;
    task.command_index = None;
    task.command_total = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mode::Mode;
    use crate::tui::state::TaskStatus;

    fn state_with(names: &[&str]) -> UiState {
        UiState::new(
            names.iter().map(|s| (*s).to_string()).collect(),
            Mode::Install,
        )
    }

    #[test]
    fn burst_no_thrash_while_selected_still_running() {
        let mut state = state_with(&["a", "b", "c"]);
        {
            let t = find_or_create_task(&mut state, "a");
            t.mark_running();
        }
        state.selected = 0;
        after_engine_event(
            &mut state,
            BurstContext {
                running_before: 0,
                running_after: 1,
                task_name: "a".into(),
                soft: SoftSelect::Prefer("a".into()),
                task_failed_during_burst: false,
            },
        );
        {
            let t = find_or_create_task(&mut state, "b");
            t.mark_running();
        }
        after_engine_event(
            &mut state,
            BurstContext {
                running_before: 1,
                running_after: 2,
                task_name: "b".into(),
                soft: SoftSelect::Prefer("b".into()),
                task_failed_during_burst: false,
            },
        );
        assert_eq!(state.selected, 0);
        assert!(state.in_burst_mode());
    }

    #[test]
    fn burst_leave_prefers_failed() {
        let mut state = state_with(&["a", "b"]);
        for name in ["a", "b"] {
            let t = find_or_create_task(&mut state, name);
            t.mark_running();
        }
        state.selected = 0;
        {
            let t = &mut state.tasks[1];
            t.status = TaskStatus::Failed("boom".into());
        }
        after_engine_event(
            &mut state,
            BurstContext {
                running_before: 2,
                running_after: 1,
                task_name: "b".into(),
                soft: SoftSelect::AnyRunning,
                task_failed_during_burst: true,
            },
        );
        assert_eq!(state.selected, 1);
        assert!(!state.log_follow);
        assert!(!state.in_burst_mode());
    }
}
