//! Command kind catalog — sole owner of behavioral `match`es on [`CommandEntry`].
//!
//! Deserialize may match YAML keys only to *construct* the enum (`config::types`).
//! Everything else that branches on kind (executor factory, validate, sudo,
//! display) lives here (ADR-0006 / CONTEXT.md **Command kind catalog**).

use std::path::Path;

use crate::config::types::{AppConfig, CommandEntry};
use crate::engine::concurrency::ExclusiveLane;
use crate::engine::mode::Mode;
use crate::utils::shell::validate_env_key;

use super::clone::CloneCommand;
use super::copy::CopyCommand;
use super::run::RunCommand;
use super::setup::SetupCommand;
use super::symlink::SymlinkCommand;
use super::CommandExecutor;

/// YAML/JSON keys for Command entry kinds — single list for schema generation
/// and authoring docs. Keep in sync with `CommandEntry` Deserialize.
pub const KIND_KEYS: &[&str] = &["copy", "symlink", "clone", "run", "machine_setup"];

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

/// Infer an Exclusive lane from a `run` Command entry's script for this Mode.
///
/// Authors do not declare lanes. First matching family in table order wins.
pub fn exclusive_lane(entry: &CommandEntry, mode: Mode) -> Option<ExclusiveLane> {
    let CommandEntry::Run(args) = entry else {
        return None;
    };
    let scripts = args.commands_for_mode(mode);
    FAMILIES
        .iter()
        .find(|(_, tokens)| {
            scripts
                .iter()
                .any(|script| tokens.iter().any(|token| script_has_token(script, token)))
        })
        .map(|(lane, _)| *lane)
}

/// Stable family order — first match is the lane (ADR-0010).
const FAMILIES: &[(ExclusiveLane, &[&str])] = &[
    (ExclusiveLane::Apt, &["apt", "apt-get", "aptitude", "dpkg"]),
    (ExclusiveLane::Brew, &["brew"]),
    (ExclusiveLane::Dnf, &["dnf", "yum"]),
    (ExclusiveLane::Pacman, &["pacman", "yay", "paru"]),
    (ExclusiveLane::Apk, &["apk"]),
    (ExclusiveLane::Winget, &["winget"]),
    (ExclusiveLane::Choco, &["choco", "chocolatey"]),
];

fn script_has_token(script: &str, token: &str) -> bool {
    script
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .any(|word| word == token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::CommandEntry;

    fn parse_entry(yaml: &str) -> CommandEntry {
        serde_yaml::from_str(yaml).expect("command entry yaml")
    }

    #[test]
    fn apt_get_joins_apt_lane() {
        let entry = parse_entry("run:\n  commands: sudo apt-get install git");
        assert_eq!(
            exclusive_lane(&entry, Mode::Install),
            Some(ExclusiveLane::Apt)
        );
    }

    #[test]
    fn echo_does_not_join_a_lane() {
        let entry = parse_entry("run:\n  commands: echo hello");
        assert_eq!(exclusive_lane(&entry, Mode::Install), None);
    }

    #[test]
    fn brew_joins_brew_lane() {
        let entry = parse_entry("run:\n  commands: brew install git");
        assert_eq!(
            exclusive_lane(&entry, Mode::Install),
            Some(ExclusiveLane::Brew)
        );
    }

    #[test]
    fn first_family_wins_on_dual_pm_script() {
        let entry = parse_entry("run:\n  commands: apt install foo && brew install bar");
        assert_eq!(
            exclusive_lane(&entry, Mode::Install),
            Some(ExclusiveLane::Apt)
        );
    }

    #[test]
    fn aptitude_joins_apt_lane() {
        let entry = parse_entry("run:\n  commands: sudo aptitude install git");
        assert_eq!(
            exclusive_lane(&entry, Mode::Install),
            Some(ExclusiveLane::Apt)
        );
    }

    #[test]
    fn adaptive_is_not_apt() {
        let entry = parse_entry("run:\n  commands: echo adaptive");
        assert_eq!(exclusive_lane(&entry, Mode::Install), None);
    }

    #[test]
    fn copy_entry_has_no_lane() {
        let entry = parse_entry("copy:\n  src: /tmp/a\n  target: /tmp/b");
        assert_eq!(exclusive_lane(&entry, Mode::Install), None);
    }

    #[test]
    fn unused_install_apt_does_not_join_on_update() {
        let entry = parse_entry("run:\n  install: sudo apt-get install git");
        assert_eq!(exclusive_lane(&entry, Mode::Update), None);
        assert_eq!(
            exclusive_lane(&entry, Mode::Install),
            Some(ExclusiveLane::Apt)
        );
    }
}
