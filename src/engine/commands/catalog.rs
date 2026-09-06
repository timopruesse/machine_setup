//! Command kind catalog — sole owner of behavioral `match`es on [`CommandEntry`].
//!
//! Deserialize may match YAML keys only to *construct* the enum (`config::types`).
//! Everything else that branches on kind (executor factory, validate, sudo,
//! display) lives here (ADR-0006 / CONTEXT.md **Command kind catalog**).

use std::path::Path;

use crate::config::types::{AppConfig, CommandEntry, RunArgs, StringOrVec};
use crate::engine::concurrency::ExclusiveLane;
use crate::engine::mode::Mode;
use crate::utils::shell::validate_env_key;

use super::clone::CloneCommand;
use super::copy::CopyCommand;
use super::run::RunCommand;
use super::setup::SetupCommand;
use super::symlink::SymlinkCommand;
use super::Executor;

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

/// Create a Command executor from a Command entry (closed enum — ADR-0006).
pub fn create_executor(entry: &CommandEntry) -> Executor {
    match entry {
        CommandEntry::Copy(args) => Executor::Copy(CopyCommand::new(args.clone())),
        CommandEntry::Symlink(args) => Executor::Symlink(SymlinkCommand::new(args.clone())),
        CommandEntry::Clone(args) => Executor::Clone(CloneCommand::new(args.clone())),
        CommandEntry::Run(args) => Executor::Run(RunCommand::new(args.clone())),
        CommandEntry::MachineSetup(args) => Executor::MachineSetup(SetupCommand::new(args.clone())),
    }
}

/// Display label for a Command entry (used by `Display` and Task events).
pub fn description(entry: &CommandEntry) -> std::sync::Arc<str> {
    match entry {
        CommandEntry::Copy(args) => std::sync::Arc::from(args.to_string()),
        CommandEntry::Symlink(args) => std::sync::Arc::from(args.to_string()),
        CommandEntry::Clone(args) => std::sync::Arc::from(args.to_string()),
        CommandEntry::Run(args) => std::sync::Arc::from(args.to_string()),
        CommandEntry::MachineSetup(args) => std::sync::Arc::from(args.to_string()),
    }
}

/// Whether this Command entry needs elevated privileges for the selected tasks UI.
pub fn entry_requires_sudo(entry: &CommandEntry) -> bool {
    if !entry.os().matches_current() {
        return false;
    }
    match entry {
        CommandEntry::Run(args) => args.all_command_strings().any(|s| s.contains("sudo")),
        CommandEntry::Copy(args) => args.sudo,
        CommandEntry::Symlink(args) => args.sudo,
        CommandEntry::Clone(_) | CommandEntry::MachineSetup(_) => false,
    }
}

/// Demote sudo on one Command entry for unattended (non-interactive) execution.
///
/// Mutates `entry` in place and appends human-readable warning lines to `warnings`.
pub fn demote_entry_for_unattended(
    task_name: &str,
    entry: &mut CommandEntry,
    warnings: &mut Vec<String>,
) {
    match entry {
        CommandEntry::Copy(args) if args.sudo => {
            args.sudo = false;
            warnings.push(format!(
                "task `{task_name}`: schedule run demoted copy sudo (running without privileges)"
            ));
        }
        CommandEntry::Symlink(args) if args.sudo => {
            args.sudo = false;
            warnings.push(format!(
                "task `{task_name}`: schedule run demoted symlink sudo (running without privileges)"
            ));
        }
        CommandEntry::Run(args) => {
            let before = args.all_command_strings().any(|s| s.contains("sudo"));
            if !before {
                return;
            }
            strip_run_fields(args);
            warnings.push(format!(
                "task `{task_name}`: schedule run stripped leading sudo from run commands (running without privileges)"
            ));
            if args.all_command_strings().any(|s| s.contains("sudo")) {
                warnings.push(format!(
                    "task `{task_name}`: residual `sudo` remains in run commands after demotion; update may still fail"
                ));
            }
        }
        _ => {}
    }
}

fn strip_run_fields(args: &mut RunArgs) {
    strip_string_or_vec(&mut args.commands);
    strip_string_or_vec(&mut args.install);
    strip_string_or_vec(&mut args.update);
    strip_string_or_vec(&mut args.uninstall);
}

fn strip_string_or_vec(v: &mut StringOrVec) {
    for s in v.as_mut_slice() {
        *s = strip_sudo_prefixes(s);
    }
}

/// Strip repeated leading `sudo [flags…]` prefixes from a command string.
///
/// Does not rewrite mid-string mentions (e.g. `echo sudo`).
pub fn strip_sudo_prefixes(s: &str) -> String {
    let mut current = s.trim_start();
    while let Some(next) = strip_one_leading_sudo(current) {
        current = next.trim_start();
    }
    current.to_string()
}

/// Strip one leading `sudo` plus optional short/long flags. `None` if no leading sudo.
fn strip_one_leading_sudo(s: &str) -> Option<&str> {
    let s = s.trim_start();
    let rest = s.strip_prefix("sudo")?;
    if rest.is_empty() {
        return Some("");
    }
    // Require whitespace so `sudoku` is not treated as sudo.
    let mut rest = match rest.chars().next() {
        Some(c) if c.is_whitespace() => rest.trim_start(),
        _ => return None,
    };

    loop {
        if rest.is_empty() {
            return Some("");
        }

        // End-of-options `--`
        if rest == "--" {
            return Some("");
        }
        if let Some(after) = rest.strip_prefix("--") {
            if after.is_empty() || after.starts_with(|c: char| c.is_whitespace()) {
                return Some(after.trim_start());
            }
            // Long option `--foo` / `--foo=bar`
            let opt_end = after
                .find(|c: char| c.is_whitespace())
                .unwrap_or(after.len());
            rest = after[opt_end..].trim_start();
            continue;
        }

        if rest.starts_with('-') {
            // Short option cluster (`-n`, `-nE`, …)
            let after = &rest[1..];
            if after.is_empty() {
                return Some("");
            }
            let opt_end = after
                .find(|c: char| c.is_whitespace())
                .unwrap_or(after.len());
            rest = after[opt_end..].trim_start();
            continue;
        }

        return Some(rest);
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
    use crate::config::types::{
        CommandEntry, CopyArgs, OsFilter, RunArgs, StringOrVec, SymlinkArgs,
    };
    use std::collections::HashMap;

    fn parse_entry(yaml: &str) -> CommandEntry {
        serde_yaml::from_str(yaml).expect("command entry yaml")
    }

    #[test]
    fn strip_plain_sudo() {
        assert_eq!(strip_sudo_prefixes("sudo apt update"), "apt update");
    }

    #[test]
    fn strip_sudo_n() {
        assert_eq!(strip_sudo_prefixes("sudo -n apt update"), "apt update");
    }

    #[test]
    fn strip_multiple_flags() {
        assert_eq!(strip_sudo_prefixes("sudo -n -E apt update"), "apt update");
    }

    #[test]
    fn strip_end_of_options() {
        assert_eq!(strip_sudo_prefixes("sudo -- apt update"), "apt update");
    }

    #[test]
    fn strip_long_option() {
        assert_eq!(
            strip_sudo_prefixes("sudo --non-interactive apt update"),
            "apt update"
        );
    }

    #[test]
    fn no_leading_sudo_unchanged() {
        assert_eq!(strip_sudo_prefixes("apt update"), "apt update");
    }

    #[test]
    fn mid_string_sudo_unchanged() {
        assert_eq!(strip_sudo_prefixes("echo sudo"), "echo sudo");
    }

    #[test]
    fn sudoku_not_stripped() {
        assert_eq!(strip_sudo_prefixes("sudoku install"), "sudoku install");
    }

    #[test]
    fn repeated_sudo_stripped() {
        assert_eq!(strip_sudo_prefixes("sudo sudo apt"), "apt");
    }

    #[test]
    fn demote_entry_clears_copy_symlink_and_strips_run() {
        let mut warnings = Vec::new();

        let mut copy = CommandEntry::Copy(CopyArgs {
            src: "/a".into(),
            target: "/b".into(),
            ignore: vec![],
            sudo: true,
            os: OsFilter::All,
        });
        demote_entry_for_unattended("priv", &mut copy, &mut warnings);
        match &copy {
            CommandEntry::Copy(a) => assert!(!a.sudo),
            _ => panic!("expected copy"),
        }

        let mut symlink = CommandEntry::Symlink(SymlinkArgs {
            src: "/a".into(),
            target: "/b".into(),
            ignore: vec![],
            force: false,
            sudo: true,
            os: OsFilter::All,
            backup: false,
        });
        demote_entry_for_unattended("priv", &mut symlink, &mut warnings);
        match &symlink {
            CommandEntry::Symlink(a) => assert!(!a.sudo),
            _ => panic!("expected symlink"),
        }

        let mut run = CommandEntry::Run(RunArgs {
            commands: StringOrVec::default(),
            install: StringOrVec::default(),
            update: serde_yaml::from_str(r#""sudo apt update""#).unwrap(),
            uninstall: StringOrVec::default(),
            shell: None,
            env: HashMap::new(),
            quiet: false,
            os: OsFilter::All,
        });
        demote_entry_for_unattended("priv", &mut run, &mut warnings);
        match &run {
            CommandEntry::Run(a) => {
                assert_eq!(
                    a.commands_for_mode(crate::engine::mode::Mode::Update),
                    &["apt update"]
                );
            }
            _ => panic!("expected run"),
        }

        assert!(!warnings.is_empty());
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
