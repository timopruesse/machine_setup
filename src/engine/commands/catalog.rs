//! Command kind catalog — sole owner of behavioral `match`es on [`CommandEntry`].
//!
//! Deserialize may match YAML keys only to *construct* the enum (`config::types`).
//! Everything else that branches on kind (executor factory, validate, sudo,
//! display) lives here (ADR-0006 / CONTEXT.md **Command kind catalog**).

use std::path::Path;

use crate::config::types::{AppConfig, CommandEntry};
use crate::utils::shell::validate_env_key;

use super::clone::CloneCommand;
use super::copy::CopyCommand;
use super::run::RunCommand;
use super::setup::SetupCommand;
use super::symlink::SymlinkCommand;
use super::CommandExecutor;

/// Severity for kind-level validation notes (mapped by `config::validate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindSeverity {
    Error,
    Warning,
}

/// One validation note for a single Command entry.
#[derive(Debug, Clone)]
pub struct KindIssue {
    pub message: String,
    pub severity: KindSeverity,
}

/// Create a Command executor from a Command entry.
pub fn create_executor(entry: CommandEntry) -> Box<dyn CommandExecutor> {
    match entry {
        CommandEntry::Copy(args) => Box::new(CopyCommand::new(args)),
        CommandEntry::Symlink(args) => Box::new(SymlinkCommand::new(args)),
        CommandEntry::Clone(args) => Box::new(CloneCommand::new(args)),
        CommandEntry::Run(args) => Box::new(RunCommand::new(args)),
        CommandEntry::MachineSetup(args) => Box::new(SetupCommand::new(args)),
    }
}

/// Display label for a Command entry (used by `Display` and Task events).
pub fn description(entry: &CommandEntry) -> String {
    match entry {
        CommandEntry::Copy(args) => args.to_string(),
        CommandEntry::Symlink(args) => args.to_string(),
        CommandEntry::Clone(args) => args.to_string(),
        CommandEntry::Run(args) => args.to_string(),
        CommandEntry::MachineSetup(args) => args.to_string(),
    }
}

/// Whether this Command entry needs elevated privileges for the selected tasks UI.
pub fn entry_requires_sudo(entry: &CommandEntry) -> bool {
    match entry {
        CommandEntry::Run(args) => args.all_command_strings().any(|s| s.contains("sudo")),
        CommandEntry::Copy(args) => args.sudo,
        CommandEntry::Symlink(args) => args.sudo,
        CommandEntry::Clone(_) | CommandEntry::MachineSetup(_) => false,
    }
}

/// True if any Command entry in the named tasks requires sudo.
pub fn tasks_require_sudo(config: &AppConfig, task_names: &[String]) -> bool {
    let selected: std::collections::HashSet<&str> = task_names.iter().map(String::as_str).collect();
    config
        .tasks
        .iter()
        .filter(|(name, _)| selected.contains(name.as_str()))
        .any(|(_, task)| task.commands.iter().any(entry_requires_sudo))
}

/// Kind-specific checks for one Command entry.
pub fn validate_entry(entry: &CommandEntry, config_dir: &Path) -> Vec<KindIssue> {
    let mut issues = Vec::new();
    match entry {
        CommandEntry::Run(args) => {
            if args.all_command_strings().next().is_none() {
                issues.push(KindIssue {
                    message: format!("Run command has no commands defined: {entry}"),
                    severity: KindSeverity::Warning,
                });
            }
            for key in args.env.keys() {
                if !validate_env_key(key) {
                    issues.push(KindIssue {
                        message: format!("Invalid environment variable name: {key:?}"),
                        severity: KindSeverity::Error,
                    });
                }
            }
        }
        CommandEntry::Copy(args) => {
            let src = crate::utils::path::expand_path(&args.src, Some(config_dir));
            if !src.exists() {
                issues.push(KindIssue {
                    message: format!("Copy source does not exist: {}", src.display()),
                    severity: KindSeverity::Warning,
                });
            }
        }
        CommandEntry::Symlink(args) => {
            let src = crate::utils::path::expand_path(&args.src, Some(config_dir));
            if !src.exists() {
                issues.push(KindIssue {
                    message: format!("Symlink source does not exist: {}", src.display()),
                    severity: KindSeverity::Warning,
                });
            }
        }
        CommandEntry::MachineSetup(args) => {
            if !crate::config::is_url(&args.config) {
                let path = crate::utils::path::expand_path(&args.config, Some(config_dir));
                let exists = path.exists()
                    || path.with_extension("yaml").exists()
                    || path.with_extension("yml").exists()
                    || path.with_extension("json").exists();
                if !exists {
                    issues.push(KindIssue {
                        message: format!("Sub-config not found: {}", path.display()),
                        severity: KindSeverity::Error,
                    });
                }
            }
        }
        CommandEntry::Clone(_) => {}
    }
    issues
}
