use crate::cli::Command;
use serde::{Deserialize, Serialize};

/// The execution modes the engine actually acts on.
///
/// Distinct from the CLI [`Command`], which also carries non-execution verbs
/// (`list`, `validate`, `init`, `add`, `schema`, `completions`) that never
/// reach the engine. Keeping `Mode` separate means every match over an
/// execution mode is exhaustive in three real arms instead of carrying dead
/// arms for verbs that can't occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Install,
    Update,
    Uninstall,
}

impl Mode {
    /// Map a CLI command to an execution mode. Returns `None` for verbs that
    /// don't drive task execution; the caller handles those before the engine
    /// is ever constructed.
    pub fn from_command(command: &Command) -> Option<Self> {
        match command {
            Command::Install => Some(Mode::Install),
            Command::Update => Some(Mode::Update),
            Command::Uninstall => Some(Mode::Uninstall),
            Command::List
            | Command::Validate
            | Command::Doctor { .. }
            | Command::Init
            | Command::Wizard
            | Command::Add { .. }
            | Command::Remove { .. }
            | Command::Schedule { .. }
            | Command::Schema
            | Command::Completions { .. } => None,
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Install => write!(f, "install"),
            Mode::Update => write!(f, "update"),
            Mode::Uninstall => write!(f, "uninstall"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::AddTarget;

    #[test]
    fn test_from_command_execution_verbs() {
        assert_eq!(Mode::from_command(&Command::Install), Some(Mode::Install));
        assert_eq!(Mode::from_command(&Command::Update), Some(Mode::Update));
        assert_eq!(
            Mode::from_command(&Command::Uninstall),
            Some(Mode::Uninstall)
        );
    }

    #[test]
    fn test_from_command_non_execution_verbs() {
        assert_eq!(Mode::from_command(&Command::List), None);
        assert_eq!(Mode::from_command(&Command::Validate), None);
        assert_eq!(Mode::from_command(&Command::Doctor { fix: false }), None);
        assert_eq!(Mode::from_command(&Command::Init), None);
        assert_eq!(Mode::from_command(&Command::Wizard), None);
        assert_eq!(
            Mode::from_command(&Command::Add {
                target: AddTarget::Task { name: "x".into() }
            }),
            None
        );
        assert_eq!(Mode::from_command(&Command::Schema), None);
        assert_eq!(
            Mode::from_command(&Command::Completions {
                shell: clap_complete::Shell::Bash
            }),
            None
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(Mode::Install.to_string(), "install");
        assert_eq!(Mode::Update.to_string(), "update");
        assert_eq!(Mode::Uninstall.to_string(), "uninstall");
    }
}
