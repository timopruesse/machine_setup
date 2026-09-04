use std::path::PathBuf;
use std::sync::Arc;

use crate::config::types::Shell;
use crate::engine::event::TaskEvent;
use crate::engine::output::{sanitize_subprocess_line, OutputKind};
use crate::engine::sink::{SharedSink, TaskEventSink};

use super::concurrency::ConcurrencyGate;
use super::mode::Mode;

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
    pub config_dir: Arc<PathBuf>,

    /// Temp directory for scripts and history.
    pub temp_dir: Arc<PathBuf>,

    /// Default shell from config.
    pub default_shell: Shell,

    /// Name of the current task being executed (shared across Task events).
    pub task_name: Arc<str>,

    /// Nesting depth (0 = top-level, 1 = sub-config, etc.)
    pub depth: usize,
}

impl CommandContext {
    /// Emit a Task event through the sink.
    pub fn emit(&self, event: TaskEvent) {
        TaskEventSink::emit(self.events.as_ref(), event);
    }

    /// Subprocess stdout/stderr (sanitized).
    pub fn log(&self, line: impl Into<String>) {
        self.log_kind(OutputKind::Subprocess, line);
    }

    pub fn log_kind(&self, kind: OutputKind, line: impl Into<String>) {
        let line = match kind {
            OutputKind::Subprocess | OutputKind::SubprocessErr => {
                match sanitize_subprocess_line(line.into()) {
                    Some(s) => s,
                    None => return,
                }
            }
            _ => line.into(),
        };
        self.emit(TaskEvent::CommandOutput {
            task_name: Arc::clone(&self.task_name),
            line,
            kind,
        });
    }

    pub fn log_progress(&self, line: impl Into<String>) {
        self.log_kind(OutputKind::Progress, line);
    }

    pub fn log_info(&self, line: impl Into<String>) {
        self.log_kind(OutputKind::Info, line);
    }
}

/// Shorten absolute paths for log display (`~/…` when under the home directory).
pub fn display_path(path: &std::path::Path) -> String {
    crate::utils::path::shorten_path(path)
}
