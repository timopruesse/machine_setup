use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::cli::Command;
use crate::config::types::Shell;

use super::event::TaskEvent;

/// Context passed to each command during execution.
#[derive(Clone)]
pub struct CommandContext {
    /// Channel for sending events to the UI/logger.
    pub event_tx: mpsc::UnboundedSender<TaskEvent>,

    /// Current execution mode (install/update/uninstall).
    pub mode: Command,

    /// Directory where the config file is located (for resolving relative paths).
    pub config_dir: PathBuf,

    /// Temp directory for scripts and history.
    pub temp_dir: PathBuf,

    /// Default shell from config.
    pub default_shell: Shell,

    /// Name of the current task being executed.
    pub task_name: String,
}

impl CommandContext {
    /// Send a command output event.
    pub fn log(&self, line: impl Into<String>) {
        let _ = self.event_tx.send(TaskEvent::CommandOutput {
            task_name: self.task_name.clone(),
            line: line.into(),
        });
    }
}
