use crate::engine::event::TaskEvent;
use crate::engine::output::OutputKind;

use super::message::{Effect, Input, Message};
use super::parallel_burst::{
    self, clear_command_progress, find_or_create_task, set_command_progress, BurstContext,
    SoftSelect,
};
use super::state::{TaskStatus, UiState};

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
    let running_before = state.running_count();

    let (task_name, soft, task_failed_during_burst) = match event {
        TaskEvent::TaskStarted {
            task_name: name,
            command_count,
            depth,
        } => {
            let task = find_or_create_task(state, &name);
            task.mark_running();
            task.command_count = command_count;
            task.depth = depth;
            let line = format!("Starting ({command_count} commands)...");
            task.push_log(OutputKind::TaskStatus, line);
            state.ensure_task_color(&name);
            (name.clone(), SoftSelect::Prefer(name), false)
        }
        TaskEvent::TaskSkipped {
            task_name: name,
            reason,
        } => {
            let task = find_or_create_task(state, &name);
            task.freeze_duration();
            let reason = crate::tui::format::strip_ansi(&reason);
            task.status = TaskStatus::Skipped(reason.clone());
            let line = format!("Skipped: {reason}");
            task.push_log(OutputKind::TaskStatus, line);
            state.skipped += 1;
            (name, SoftSelect::AnyRunning, false)
        }
        TaskEvent::CommandStarted {
            task_name: name,
            command_desc,
            command_index,
            command_total,
        } => {
            let task = find_or_create_task(state, &name);
            let command_desc = crate::tui::format::strip_ansi(&command_desc);
            set_command_progress(task, command_index, command_total, &command_desc);
            task.push_log(OutputKind::CommandStart, command_desc);
            (name, SoftSelect::None, false)
        }
        TaskEvent::CommandWaiting {
            task_name: name,
            lane,
            ..
        } => {
            let task = find_or_create_task(state, &name);
            task.push_log(OutputKind::Info, format!("Waiting for {lane}"));
            (name, SoftSelect::None, false)
        }
        TaskEvent::CommandOutput {
            task_name: name,
            line,
            kind,
        } => {
            let task = find_or_create_task(state, &name);
            task.push_log(kind, line);
            (name, SoftSelect::None, false)
        }
        TaskEvent::CommandCompleted {
            task_name: name,
            command_desc,
            command_index,
            command_total,
        } => {
            let task = find_or_create_task(state, &name);
            clear_command_progress(task);
            let line = format!("{command_desc} ({command_index}/{command_total})");
            task.push_log(OutputKind::CommandDone, line);
            (name, SoftSelect::None, false)
        }
        TaskEvent::CommandFailed {
            task_name: name,
            command_desc,
            command_index,
            command_total,
            error,
        } => {
            let task = find_or_create_task(state, &name);
            clear_command_progress(task);
            let line = format!("{command_desc} ({command_index}/{command_total}): {error}");
            task.push_log(OutputKind::CommandFailed, line);
            (name, SoftSelect::None, false)
        }
        TaskEvent::TaskCompleted { task_name: name } => {
            let task = find_or_create_task(state, &name);
            task.freeze_duration();
            task.status = TaskStatus::Completed;
            clear_command_progress(task);
            task.push_log(OutputKind::TaskStatus, "Completed successfully.".into());
            state.succeeded += 1;
            (name, SoftSelect::AnyRunning, false)
        }
        TaskEvent::TaskFailed {
            task_name: name,
            error,
        } => {
            let task = find_or_create_task(state, &name);
            task.freeze_duration();
            let error = crate::tui::format::strip_ansi(&error);
            task.status = TaskStatus::Failed(error.clone());
            clear_command_progress(task);
            let line = format!("Failed: {error}");
            task.push_log(OutputKind::TaskStatus, line);
            state.failed += 1;
            (name, SoftSelect::AnyRunning, running_before >= 2)
        }
        TaskEvent::TaskRetry {
            task_name: name,
            attempt,
            max_attempts,
            error,
        } => {
            let task = find_or_create_task(state, &name);
            task.mark_running();
            let line = format!("Retry {attempt}/{max_attempts}: {error}");
            task.push_log(OutputKind::TaskStatus, line);
            state.ensure_task_color(&name);
            (name.clone(), SoftSelect::Prefer(name), false)
        }
        TaskEvent::AllDone { .. } => {
            state.done = true;
            state.freeze_run_elapsed();
            state.burst_failed.clear();
            state.details_expanded = false;
            if let Some(idx) = state.tasks.iter().position(|t| t.status.is_failed()) {
                state.search_mode = false;
                state.search_query.clear();
                update_filter(state);
                state.selected = idx;
                state.log_follow = false;
                if let Some(task) = state.tasks.get(idx) {
                    state.log_scroll = task.log_lines.len().saturating_sub(1);
                }
            }
            return;
        }
    };

    let running_after = state.running_count();
    parallel_burst::after_engine_event(
        state,
        BurstContext {
            running_before,
            running_after,
            task_name,
            soft,
            task_failed_during_burst,
        },
    );
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
        Input::ToggleDetailsExpand => {
            if state.in_burst_mode() {
                state.details_expanded = !state.details_expanded;
                state.log_scroll = 0;
                state.log_follow = true;
                parallel_burst::sync_log_scroll(&mut state);
            }
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
            parallel_burst::maybe_reenable_follow_at_bottom(&mut state);
            (state, Effect::None)
        }
        Input::LogHome => {
            state.log_follow = false;
            state.log_scroll = 0;
            (state, Effect::None)
        }
        Input::LogEnd => {
            state.log_follow = true;
            parallel_burst::stick_log_to_end(&mut state);
            (state, Effect::None)
        }
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
    state.log_scroll = 0;
    parallel_burst::sync_log_scroll(state);
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
    state.log_scroll = 0;
    parallel_burst::sync_log_scroll(state);
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
        assert!(state.tasks[1].started_at.is_some());
        assert!(state.tasks[1].duration.is_none());
        assert!(state.log_follow);
    }

    #[test]
    fn command_waiting_appends_info_line() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandStarted {
                task_name: "a".into(),
                command_desc: "apt".into(),
                command_index: 1,
                command_total: 1,
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandWaiting {
                task_name: "a".into(),
                command_desc: "apt".into(),
                command_index: 1,
                command_total: 1,
                lane: crate::engine::concurrency::ExclusiveLane::Apt,
            }),
        );
        let last = state.tasks[0].log_lines.last().expect("log");
        assert_eq!(last.kind, OutputKind::Info);
        assert!(last.text.contains("Waiting for apt"));
    }

    #[test]
    fn command_started_sets_progress() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandStarted {
                task_name: "a".into(),
                command_desc: "install".into(),
                command_index: 2,
                command_total: 5,
            }),
        );
        assert_eq!(state.tasks[0].command_index, Some(2));
        assert_eq!(state.tasks[0].command_total, Some(5));
        assert_eq!(state.tasks[0].current_command.as_deref(), Some("install"));
    }

    #[test]
    fn task_completed_freezes_duration() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskStarted {
                task_name: "a".into(),
                command_count: 1,
                depth: 0,
            }),
        );
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskCompleted {
                task_name: "a".into(),
            }),
        );
        assert_eq!(state.tasks[0].status, TaskStatus::Completed);
        assert!(state.tasks[0].duration.is_some());
    }

    #[test]
    fn skip_without_start_leaves_duration_unset() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskSkipped {
                task_name: "a".into(),
                reason: "wrong os".into(),
            }),
        );
        assert!(matches!(state.tasks[0].status, TaskStatus::Skipped(_)));
        assert!(state.tasks[0].started_at.is_none());
        assert!(state.tasks[0].duration.is_none());
    }

    #[test]
    fn all_done_freezes_run_elapsed() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::AllDone {
                succeeded: 0,
                failed: 0,
                skipped: 0,
            }),
        );
        assert!(state.done);
        assert!(state.run_elapsed.is_some());
    }

    #[test]
    fn command_output_ansi_stripped_in_log() {
        let state = state_with(&["a"]);
        let colored = "\u{1b}[1m\u{1b}[33mzsh-users/zsh-autosuggestions:\u{1b}[39m\u{1b}[0m";
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::CommandOutput {
                task_name: "a".into(),
                line: colored.into(),
                kind: OutputKind::Subprocess,
            }),
        );
        assert_eq!(
            state.tasks[0].log_lines.last().map(|l| l.text.as_str()),
            Some("zsh-users/zsh-autosuggestions:")
        );
        let stored = &state.tasks[0].log_lines.last().unwrap().text;
        assert!(!stored.contains("[1m") && !stored.contains("[33m"));
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
                    kind: OutputKind::Subprocess,
                }),
            );
            state = s;
        }
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
                kind: OutputKind::Subprocess,
            }),
        );
        assert!(!state.log_follow);
        assert_eq!(state.log_scroll, scrolled);
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
        assert_eq!(state.selected, 1);
        assert!(!state.log_follow);
    }

    #[test]
    fn log_cap_drops_oldest() {
        use crate::tui::state::LOG_CAP;
        let mut state = state_with(&["a"]);
        for i in 0..(LOG_CAP + 50) {
            let (s, _) = reduce(
                state,
                Message::Engine(TaskEvent::CommandOutput {
                    task_name: "a".into(),
                    line: format!("{i}"),
                    kind: OutputKind::Subprocess,
                }),
            );
            state = s;
        }
        assert_eq!(state.tasks[0].log_lines.len(), LOG_CAP);
    }

    #[test]
    fn manual_select_disables_auto_select_running() {
        let state = state_with(&["a", "b", "c"]);
        let (state, _) = reduce(state, Message::Input(Input::SelectNext));
        assert!(!state.auto_select_running);
    }

    fn start_task(state: UiState, name: &str) -> UiState {
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskStarted {
                task_name: name.into(),
                command_count: 1,
                depth: 0,
            }),
        );
        state
    }

    #[test]
    fn burst_enters_on_second_running_task() {
        let state = state_with(&["a", "b"]);
        let state = start_task(state, "a");
        assert!(!state.in_burst_mode());
        let state = start_task(state, "b");
        assert!(state.in_burst_mode());
        assert!(!state.details_expanded);
    }

    #[test]
    fn burst_toggle_expand() {
        let state = state_with(&["a", "b"]);
        let state = start_task(start_task(state, "a"), "b");
        let (state, _) = reduce(state, Message::Input(Input::ToggleDetailsExpand));
        assert!(state.details_expanded);
        let (state, _) = reduce(state, Message::Input(Input::ToggleDetailsExpand));
        assert!(!state.details_expanded);
    }

    #[test]
    fn burst_expand_ignored_when_not_in_burst() {
        let state = state_with(&["a"]);
        let (state, _) = reduce(state, Message::Input(Input::ToggleDetailsExpand));
        assert!(!state.details_expanded);
    }

    #[test]
    fn burst_leave_prefers_failed_in_burst() {
        let state = state_with(&["a", "b"]);
        let state = start_task(start_task(state, "a"), "b");
        let (state, _) = reduce(
            state,
            Message::Engine(TaskEvent::TaskFailed {
                task_name: "b".into(),
                error: "boom".into(),
            }),
        );
        assert!(!state.in_burst_mode());
        assert_eq!(state.selected, 1);
        assert!(!state.log_follow);
    }

    #[test]
    fn burst_selection_keeps_burst_mode() {
        let state = state_with(&["a", "b"]);
        let state = start_task(start_task(state, "a"), "b");
        let (state, _) = reduce(state, Message::Input(Input::SelectNext));
        assert_eq!(state.selected, 1);
        assert!(state.in_burst_mode());
    }
}
