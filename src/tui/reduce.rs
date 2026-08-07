use crate::engine::event::TaskEvent;

use super::message::{Effect, Input, Message};
use super::state::{TaskState, TaskStatus, UiState};

/// Pure state transition. No I/O.
pub fn reduce(mut state: UiState, msg: Message) -> (UiState, Effect) {
    match msg {
        Message::Tick => {
            state.tick = state.tick.wrapping_add(1);
            (state, Effect::None)
        }
        Message::Engine(event) => {
            apply_engine(&mut state, event);
            (state, Effect::None)
        }
        Message::Input(input) => apply_input(state, input),
    }
}

fn apply_engine(state: &mut UiState, event: TaskEvent) {
    match event {
        TaskEvent::TaskStarted {
            task_name,
            command_count,
            depth,
        } => {
            let task = find_or_create_task(state, &task_name);
            task.status = TaskStatus::Running;
            task.command_count = command_count;
            task.depth = depth;
            task.push_log(format!("Starting ({command_count} commands)..."));
            let name = task_name;
            if state.auto_select_running {
                select_task(state, &name);
            }
            follow_selected_log(state);
        }
        TaskEvent::TaskSkipped { task_name, reason } => {
            let task = find_or_create_task(state, &task_name);
            task.status = TaskStatus::Skipped(reason.clone());
            task.push_log(format!("Skipped: {reason}"));
            state.skipped += 1;
            follow_selected_log(state);
        }
        TaskEvent::CommandStarted {
            task_name,
            command_desc,
        } => {
            let task = find_or_create_task(state, &task_name);
            task.current_command = Some(command_desc.clone());
            task.push_log(format!("> {command_desc}"));
            follow_selected_log(state);
        }
        TaskEvent::CommandOutput { task_name, line } => {
            let task = find_or_create_task(state, &task_name);
            task.push_log(format!("  {line}"));
            follow_selected_log(state);
        }
        TaskEvent::CommandCompleted {
            task_name,
            command_desc,
        } => {
            let task = find_or_create_task(state, &task_name);
            task.current_command = None;
            task.push_log(format!("  [done] {command_desc}"));
            follow_selected_log(state);
        }
        TaskEvent::CommandFailed {
            task_name,
            command_desc,
            error,
        } => {
            let task = find_or_create_task(state, &task_name);
            task.current_command = None;
            task.push_log(format!("  [FAILED] {command_desc}: {error}"));
            follow_selected_log(state);
        }
        TaskEvent::TaskCompleted { task_name } => {
            let task = find_or_create_task(state, &task_name);
            task.status = TaskStatus::Completed;
            task.push_log("Completed successfully.".to_string());
            state.succeeded += 1;
            follow_selected_log(state);
        }
        TaskEvent::TaskFailed { task_name, error } => {
            let task = find_or_create_task(state, &task_name);
            task.status = TaskStatus::Failed(error.clone());
            task.push_log(format!("FAILED: {error}"));
            state.failed += 1;
            follow_selected_log(state);
        }
        TaskEvent::TaskRetry {
            task_name,
            attempt,
            max_attempts,
            error,
        } => {
            let task = find_or_create_task(state, &task_name);
            task.status = TaskStatus::Running;
            task.push_log(format!("  Retry {attempt}/{max_attempts}: {error}"));
            follow_selected_log(state);
        }
        TaskEvent::AllDone { .. } => {
            state.done = true;
            if let Some(idx) = state.tasks.iter().position(|t| t.status.is_failed()) {
                // Clear any active filter so the jumped failure is visible in the list.
                state.search_mode = false;
                state.search_query.clear();
                update_filter(state);
                state.selected = idx;
                state.log_follow = false;
                if let Some(task) = state.tasks.get(idx) {
                    state.log_scroll = task.log_lines.len().saturating_sub(1);
                }
            }
        }
    }
}

fn apply_input(mut state: UiState, input: Input) -> (UiState, Effect) {
    match input {
        Input::CancelAndQuit => (state, Effect::CancelAndQuit),
        Input::ClearFilterOrQuit => {
            if state.filter_active() {
                state.search_mode = false;
                state.search_query.clear();
                update_filter(&mut state);
                (state, Effect::None)
            } else {
                (state, Effect::Quit)
            }
        }
        Input::EnterSearch => {
            state.search_mode = true;
            state.search_query.clear();
            update_filter(&mut state);
            (state, Effect::None)
        }
        Input::ConfirmSearch => {
            state.search_mode = false;
            (state, Effect::None)
        }
        Input::ExitSearch => {
            state.search_mode = false;
            state.search_query.clear();
            update_filter(&mut state);
            (state, Effect::None)
        }
        Input::SearchChar(c) => {
            state.search_query.push(c);
            update_filter(&mut state);
            (state, Effect::None)
        }
        Input::SearchBackspace => {
            state.search_query.pop();
            update_filter(&mut state);
            (state, Effect::None)
        }
        Input::SelectNext => {
            select_next(&mut state);
            (state, Effect::None)
        }
        Input::SelectPrev => {
            select_prev(&mut state);
            (state, Effect::None)
        }
        Input::LogPageUp => {
            state.log_follow = false;
            state.log_scroll = state.log_scroll.saturating_sub(3);
            (state, Effect::None)
        }
        Input::LogPageDown => {
            state.log_scroll = state.log_scroll.saturating_add(3);
            maybe_reenable_follow_at_bottom(&mut state);
            (state, Effect::None)
        }
        Input::LogHome => {
            state.log_follow = false;
            state.log_scroll = 0;
            (state, Effect::None)
        }
        Input::LogEnd => {
            state.log_follow = true;
            stick_log_to_end(&mut state);
            (state, Effect::None)
        }
    }
}

fn find_or_create_task<'a>(state: &'a mut UiState, name: &str) -> &'a mut TaskState {
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

fn select_task(state: &mut UiState, name: &str) {
    if let Some(idx) = state.tasks.iter().position(|t| t.name == name) {
        state.selected = idx;
        state.log_scroll = 0;
        state.log_follow = true;
    }
}

fn select_next(state: &mut UiState) {
    state.auto_select_running = false;
    if state.filtered_indices.is_empty() {
        return;
    }
    if let Some(pos) = state
        .filtered_indices
        .iter()
        .position(|&i| i > state.selected)
    {
        state.selected = state.filtered_indices[pos];
    }
    state.log_follow = true;
    stick_log_to_end(state);
}

fn select_prev(state: &mut UiState) {
    state.auto_select_running = false;
    if state.filtered_indices.is_empty() {
        return;
    }
    if let Some(pos) = state
        .filtered_indices
        .iter()
        .rposition(|&i| i < state.selected)
    {
        state.selected = state.filtered_indices[pos];
    }
    state.log_follow = true;
    stick_log_to_end(state);
}

fn update_filter(state: &mut UiState) {
    let query = state.search_query.to_lowercase();
    state.filtered_indices = state
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| query.is_empty() || task.name.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();

    if !state.filtered_indices.contains(&state.selected) {
        if let Some(&first) = state.filtered_indices.first() {
            state.selected = first;
            state.log_scroll = 0;
            state.log_follow = true;
        }
    }
}

fn follow_selected_log(state: &mut UiState) {
    if state.log_follow {
        stick_log_to_end(state);
    }
}

fn stick_log_to_end(state: &mut UiState) {
    if let Some(task) = state.tasks.get(state.selected) {
        let line_count = task.log_lines.len();
        if line_count > 0 {
            state.log_scroll = line_count.saturating_sub(1);
        }
    }
}

fn maybe_reenable_follow_at_bottom(state: &mut UiState) {
    if let Some(task) = state.tasks.get(state.selected) {
        let end = task.log_lines.len().saturating_sub(1);
        if state.log_scroll >= end {
            state.log_follow = true;
            state.log_scroll = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mode::Mode;

    fn state_with(names: &[&str]) -> UiState {
        UiState::new(
            names.iter().map(|s| (*s).to_string()).collect(),
            Mode::Install,
        )
    }

    #[test]
    fn engine_task_started_marks_running_and_auto_selects() {
        let state = state_with(&["a", "b"]);
        let (state, effect) = reduce(
            state,
            Message::Engine(TaskEvent::TaskStarted {
                task_name: "b".into(),
                command_count: 2,
                depth: 1,
            }),
        );
        assert_eq!(effect, Effect::None);
        assert_eq!(state.selected, 1);
        assert_eq!(state.tasks[1].status, TaskStatus::Running);
        assert_eq!(state.tasks[1].depth, 1);
        assert_eq!(state.tasks[1].command_count, 2);
        assert!(state.log_follow);
    }

    #[test]
    fn search_enter_keeps_filter_esc_clears_without_quit() {
        let state = state_with(&["alpha", "beta", "gamma"]);
        let (state, _) = reduce(state, Message::Input(Input::EnterSearch));
        let (state, _) = reduce(state, Message::Input(Input::SearchChar('b')));
        let (state, _) = reduce(state, Message::Input(Input::SearchChar('e')));
        assert!(state.search_mode);
        assert_eq!(state.filtered_indices, vec![1]);

        let (state, effect) = reduce(state, Message::Input(Input::ConfirmSearch));
        assert_eq!(effect, Effect::None);
        assert!(!state.search_mode);
        assert_eq!(state.search_query, "be");
        assert!(state.filter_active());
        assert_eq!(state.filtered_indices, vec![1]);

        let (state, effect) = reduce(state, Message::Input(Input::ClearFilterOrQuit));
        assert_eq!(effect, Effect::None);
        assert!(!state.filter_active());
        assert_eq!(state.filtered_indices.len(), 3);
    }

    #[test]
    fn clear_filter_or_quit_quits_when_no_filter() {
        let state = state_with(&["a"]);
        let (state, effect) = reduce(state, Message::Input(Input::ClearFilterOrQuit));
        assert_eq!(effect, Effect::Quit);
        assert!(!state.filter_active());
    }

    #[test]
    fn log_page_up_disables_follow_output_does_not_snap() {
        let mut state = state_with(&["a"]);
        for i in 0..20 {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("line-{i}"),
                }),
            );
            state = s;
        }
        assert!(state.log_follow);
        let scroll_at_end = state.log_scroll;

        let (state, _) = reduce(state, Message::Input(Input::LogPageUp));
        assert!(!state.log_follow);
        assert!(state.log_scroll < scroll_at_end);
        let scrolled = state.log_scroll;

        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandOutput {
                task_name: "a".into(),
                line: "new".into(),
            }),
        );
        assert!(!state.log_follow);
        assert_eq!(state.log_scroll, scrolled);
    }

    #[test]
    fn log_end_reenables_follow() {
        let mut state = state_with(&["a"]);
        for i in 0..10 {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("line-{i}"),
                }),
            );
            state = s;
        }
        let (state, _) = reduce(state, Message::Input(Input::LogHome));
        assert!(!state.log_follow);
        assert_eq!(state.log_scroll, 0);

        let (state, _) = reduce(state, Message::Input(Input::LogEnd));
        assert!(state.log_follow);
        assert_eq!(state.log_scroll, state.tasks[0].log_lines.len() - 1);
    }

    #[test]
    fn all_done_with_failures_selects_first_failed() {
        let state = state_with(&["ok", "bad", "also"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskCompleted {
                task_name: "ok".into(),
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskFailed {
                task_name: "bad".into(),
                error: "boom".into(),
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::AllDone {
                succeeded: 1,
                failed: 1,
                skipped: 0,
            }),
        );
        assert!(state.done);
        assert_eq!(state.selected, 1);
        assert!(!state.log_follow);
    }

    #[test]
    fn log_cap_drops_oldest() {
        use super::super::state::LOG_CAP;
        let mut state = state_with(&["a"]);
        for i in 0..(LOG_CAP + 50) {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("{i}"),
                }),
            );
            state = s;
        }
        assert_eq!(state.tasks[0].log_lines.len(), LOG_CAP);
        assert!(state.tasks[0].log_lines[0].contains("50"));
        assert!(state.tasks[0]
            .log_lines
            .last()
            .unwrap()
            .contains(&(LOG_CAP + 49).to_string()));
    }

    #[test]
    fn selection_stays_within_filtered_set() {
        let state = state_with(&["alpha", "beta", "gamma"]);
        let (state, _) = reduce(state, Message::Input(Input::SelectNext));
        assert_eq!(state.selected, 1);

        let (state, _) = reduce(state, Message::Input(Input::EnterSearch));
        let (state, _) = reduce(state, Message::Input(Input::SearchChar('g')));
        assert_eq!(state.filtered_indices, vec![2]);
        assert_eq!(state.selected, 2);

        let (state, _) = reduce(state, Message::Input(Input::SelectPrev));
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn tick_increments() {
        let state = state_with(&["a"]);
        let (state, effect) = reduce(state, Message::Tick);
        assert_eq!(effect, Effect::None);
        assert_eq!(state.tick, 1);
    }

    // --- adversarial checks (verifier) ---

    #[test]
    fn empty_tasks_inputs_do_not_panic_and_esc_quits() {
        let state = UiState::new(vec![], Mode::Install);
        assert!(state.tasks.is_empty());
        assert!(state.filtered_indices.is_empty());

        let (state, effect) = reduce(state, Message::Input(Input::SelectNext));
        assert_eq!(effect, Effect::None);
        let (state, effect) = reduce(state, Message::Input(Input::SelectPrev));
        assert_eq!(effect, Effect::None);
        let (state, effect) = reduce(state, Message::Input(Input::LogPageUp));
        assert_eq!(effect, Effect::None);
        assert!(!state.log_follow);
        let (state, _) = reduce(state, Message::Input(Input::LogEnd));
        assert!(state.log_follow);
        let (state, effect) = reduce(state, Message::Input(Input::ClearFilterOrQuit));
        assert_eq!(effect, Effect::Quit);
        let (_, effect) = reduce(state, Message::Input(Input::CancelAndQuit));
        assert_eq!(effect, Effect::CancelAndQuit);
    }

    #[test]
    fn all_done_without_failures_keeps_selection_and_follow() {
        let state = state_with(&["a", "b"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskCompleted {
                task_name: "a".into(),
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskStarted {
                task_name: "b".into(),
                command_count: 1,
                depth: 0,
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskCompleted {
                task_name: "b".into(),
            }),
        );
        assert_eq!(state.selected, 1);
        assert!(state.log_follow);

        let (state, effect) = reduce(
            state,
            Message::Engine(TaskEvent::AllDone {
                succeeded: 2,
                failed: 0,
                skipped: 0,
            }),
        );
        assert_eq!(effect, Effect::None);
        assert!(state.done);
        assert_eq!(state.selected, 1);
        assert!(state.log_follow);
        assert_eq!(state.failed, 0);
    }

    #[test]
    fn cancel_and_quit_is_not_plain_quit() {
        let state = state_with(&["a"]);
        let (_, effect) = reduce(state.clone(), Message::Input(Input::CancelAndQuit));
        assert_eq!(effect, Effect::CancelAndQuit);
        let (_, effect) = reduce(state, Message::Input(Input::ClearFilterOrQuit));
        assert_eq!(effect, Effect::Quit);
        assert_ne!(Effect::Quit, Effect::CancelAndQuit);
    }

    #[test]
    fn log_cap_boundary_exact_capacity_keeps_all() {
        use crate::tui::state::LOG_CAP;
        let mut state = state_with(&["a"]);
        for i in 0..LOG_CAP {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("{i}"),
                }),
            );
            state = s;
        }
        assert_eq!(state.tasks[0].log_lines.len(), LOG_CAP);
        assert!(state.tasks[0].log_lines[0].contains('0'));
        assert!(state.tasks[0]
            .log_lines
            .last()
            .unwrap()
            .contains(&(LOG_CAP - 1).to_string()));

        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandOutput {
                task_name: "a".into(),
                line: "overflow".into(),
            }),
        );
        assert_eq!(state.tasks[0].log_lines.len(), LOG_CAP);
        assert!(state.tasks[0].log_lines[0].contains('1'));
        assert!(state.tasks[0]
            .log_lines
            .last()
            .unwrap()
            .contains("overflow"));
    }

    #[test]
    fn filter_no_matches_then_esc_clears_without_quit() {
        let mut state = state_with(&["alpha", "beta"]);
        let (s, _) = reduce(state, Message::Input(Input::EnterSearch));
        state = s;
        for c in ['z', 'z', 'z'] {
            let (s, _) = reduce(state, Message::Input(Input::SearchChar(c)));
            state = s;
        }
        assert!(state.filtered_indices.is_empty());
        assert!(state.filter_active());

        let (state, effect) = reduce(state, Message::Input(Input::ConfirmSearch));
        assert_eq!(effect, Effect::None);
        assert!(state.filter_active());
        assert!(state.filtered_indices.is_empty());

        let (state, effect) = reduce(state, Message::Input(Input::ClearFilterOrQuit));
        assert_eq!(effect, Effect::None);
        assert!(!state.filter_active());
        assert_eq!(state.filtered_indices, vec![0, 1]);
    }

    #[test]
    fn manual_select_disables_auto_select_running() {
        let state = state_with(&["a", "b", "c"]);
        let (state, _) = reduce(state, Message::Input(Input::SelectNext));
        assert!(!state.auto_select_running);
        assert_eq!(state.selected, 1);

        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskStarted {
                task_name: "c".into(),
                command_count: 1,
                depth: 0,
            }),
        );
        assert_eq!(state.selected, 1);
        assert_eq!(state.tasks[2].status, TaskStatus::Running);
    }

    #[test]
    fn page_down_at_bottom_reenables_follow() {
        let mut state = state_with(&["a"]);
        for i in 0..10 {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("line-{i}"),
                }),
            );
            state = s;
        }
        let (state, _) = reduce(state, Message::Input(Input::LogHome));
        assert!(!state.log_follow);

        // Jump near end then PageDown should stick and follow.
        let mut state = state;
        state.log_scroll = state.tasks[0].log_lines.len().saturating_sub(2);
        let (state, _) = reduce(state, Message::Input(Input::LogPageDown));
        assert!(state.log_follow);
        assert_eq!(
            state.log_scroll,
            state.tasks[0].log_lines.len().saturating_sub(1)
        );
    }

    #[test]
    fn all_done_failure_jump_visible_under_active_filter() {
        let state = state_with(&["ok", "bad"]);
        let (state, _) = reduce(state, Message::Input(Input::EnterSearch));
        let (state, _) = reduce(state, Message::Input(Input::SearchChar('o')));
        let (state, _) = reduce(state, Message::Input(Input::ConfirmSearch));
        assert_eq!(state.filtered_indices, vec![0]);
        assert_eq!(state.selected, 0);

        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskCompleted {
                task_name: "ok".into(),
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskFailed {
                task_name: "bad".into(),
                error: "x".into(),
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::AllDone {
                succeeded: 1,
                failed: 1,
                skipped: 0,
            }),
        );
        assert_eq!(state.selected, 1);
        assert!(
            state.filtered_indices.contains(&state.selected),
            "first failed task must remain visible in the filtered list; filtered={:?} selected={}",
            state.filtered_indices,
            state.selected
        );
    }

    #[test]
    fn exit_search_esc_does_not_quit() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(state, Message::Input(Input::EnterSearch));
        let (state, effect) = reduce(state, Message::Input(Input::ExitSearch));
        assert_eq!(effect, Effect::None);
        assert!(!state.search_mode);
        assert!(!state.filter_active());
    }
}
