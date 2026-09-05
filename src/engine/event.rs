use std::sync::Arc;

use crate::engine::concurrency::ExclusiveLane;

/// Events emitted by the engine during task execution.
/// These decouple the execution logic from the presentation layer (TUI/plain log).
///
/// `task_name` is an [`Arc<str>`] interned once per Task so per-line
/// [`CommandOutput`] events clone a refcount instead of reallocating the name.
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A task is about to start executing.
    TaskStarted {
        task_name: Arc<str>,
        command_count: usize,
        /// Nesting depth (0 = top-level, 1 = sub-config, etc.)
        depth: usize,
    },

    /// A task was skipped (OS mismatch or already installed).
    TaskSkipped { task_name: Arc<str>, reason: String },

    /// A command within a task produced output (sparse progress, one line).
    CommandOutput {
        task_name: Arc<str>,
        line: String,
        kind: crate::engine::output::OutputKind,
    },

    /// Coalesced subprocess output (multiple lines per flush from stream reader).
    CommandOutputBatch {
        task_name: Arc<str>,
        lines: Vec<String>,
        kind: crate::engine::output::OutputKind,
    },

    /// A command within a task started.
    CommandStarted {
        task_name: Arc<str>,
        command_desc: Arc<str>,
        /// 1-based index within the task's command list.
        command_index: usize,
        command_total: usize,
    },

    /// A command is waiting on an Exclusive lane already held in this run.
    CommandWaiting {
        task_name: Arc<str>,
        command_desc: Arc<str>,
        command_index: usize,
        command_total: usize,
        lane: ExclusiveLane,
    },

    /// A command within a task completed successfully.
    CommandCompleted {
        task_name: Arc<str>,
        command_desc: Arc<str>,
        command_index: usize,
        command_total: usize,
    },

    /// A command within a task failed.
    CommandFailed {
        task_name: Arc<str>,
        command_desc: Arc<str>,
        command_index: usize,
        command_total: usize,
        error: String,
    },

    /// A task completed all commands successfully.
    TaskCompleted { task_name: Arc<str> },

    /// A task failed.
    TaskFailed { task_name: Arc<str>, error: String },

    /// A task is being retried after failure.
    TaskRetry {
        task_name: Arc<str>,
        attempt: u32,
        max_attempts: u32,
        error: String,
    },

    /// All tasks are done.
    AllDone {
        succeeded: usize,
        failed: usize,
        skipped: usize,
    },
}
