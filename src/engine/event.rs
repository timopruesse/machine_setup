/// Events emitted by the engine during task execution.
/// These decouple the execution logic from the presentation layer (TUI/plain log).
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A task is about to start executing.
    TaskStarted {
        task_name: String,
        command_count: usize,
    },

    /// A task was skipped (OS mismatch or already installed).
    TaskSkipped { task_name: String, reason: String },

    /// A command within a task produced output.
    CommandOutput { task_name: String, line: String },

    /// A command within a task started.
    CommandStarted {
        task_name: String,
        command_desc: String,
    },

    /// A command within a task completed successfully.
    CommandCompleted {
        task_name: String,
        command_desc: String,
    },

    /// A command within a task failed.
    CommandFailed {
        task_name: String,
        command_desc: String,
        error: String,
    },

    /// A task completed all commands successfully.
    TaskCompleted { task_name: String },

    /// A task failed.
    TaskFailed { task_name: String, error: String },

    /// All tasks are done.
    AllDone {
        succeeded: usize,
        failed: usize,
        skipped: usize,
    },
}
