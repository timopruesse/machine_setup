use std::path::PathBuf;
use std::sync::Arc;

use crate::config::types::Shell;

use super::concurrency::ConcurrencyGate;
use super::event::TaskEvent;
use super::mode::Mode;
use super::sink::{SharedSink, TaskEventSink};

/// Context passed to each command during execution.
#[derive(Clone)]
pub struct CommandContext {
    /// Sink for Task events (lifecycle + command output).
    pub events: SharedSink,

    /// Global concurrency gate (Tasks + Command entries).
    pub gate: Arc<ConcurrencyGate>,

    /// Current execution mode (install/update/uninstall).
    pub mode: Mode,

    /// Directory where the config file is located (for resolving relative paths).
    pub config_dir: PathBuf,

    /// Temp directory for scripts and history.
    pub temp_dir: PathBuf,

    /// Default shell from config.
    pub default_shell: Shell,

    /// Name of the current task being executed.
    pub task_name: String,

    /// Nesting depth (0 = top-level, 1 = sub-config, etc.)
    pub depth: usize,
}

impl CommandContext {
    /// Emit a Task event through the sink.
    pub fn emit(&self, event: TaskEvent) {
        TaskEventSink::emit(self.events.as_ref(), event);
    }

    /// Send a command output event.
    pub fn log(&self, line: impl Into<String>) {
        self.emit(TaskEvent::CommandOutput {
            task_name: self.task_name.clone(),
            line: line.into(),
        });
    }
}
