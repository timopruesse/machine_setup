//! Demote sudo for non-interactive `schedule run` (no TTY / password prompt).

use std::collections::HashSet;

use crate::config::types::{AppConfig, CommandEntry, StringOrVec};
use crate::engine::commands::catalog;

/// Clone `config` and demote sudo on the named tasks for schedule execution.
///
/// Returns the demoted config and human-readable warning lines (one per change
/// or residual sudo that could not be stripped).
pub fn demote_config_for_schedule(
    config: &AppConfig,
    task_names: &[String],
) -> (AppConfig, Vec<String>) {
    let selected: HashSet<&str> = task_names.iter().map(String::as_str).collect();
    let mut demoted = config.clone();
    let mut warnings = Vec::new();

    for (name, task) in demoted.tasks.iter_mut() {
        if !selected.contains(name.as_str()) {
            continue;
        }
        if !task.commands.iter().any(catalog::entry_requires_sudo) {
            continue;
        }

        for entry in &mut task.commands {
            demote_entry(name, entry, &mut warnings);
        }
    }

    (demoted, warnings)
}

fn demote_entry(task_name: &str, entry: &mut CommandEntry, warnings: &mut Vec<String>) {
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

fn strip_run_fields(args: &mut crate::config::types::RunArgs) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{
        AutoUpdateConfig, CopyArgs, RunArgs, Shell, SymlinkArgs, TaskConfig,
    };
    use indexmap::IndexMap;
    use std::collections::HashMap;

    fn make_config(tasks: IndexMap<String, TaskConfig>) -> AppConfig {
        AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".to_string(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
            check_for_updates: true,
        }
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
    fn demote_clears_copy_symlink_and_strips_run() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "priv".to_string(),
            TaskConfig {
                commands: vec![
                    CommandEntry::Copy(CopyArgs {
                        src: "/a".into(),
                        target: "/b".into(),
                        ignore: vec![],
                        sudo: true,
                    }),
                    CommandEntry::Symlink(SymlinkArgs {
                        src: "/a".into(),
                        target: "/b".into(),
                        ignore: vec![],
                        force: false,
                        sudo: true,
                    }),
                    CommandEntry::Run(RunArgs {
                        commands: StringOrVec::default(),
                        install: StringOrVec::default(),
                        update: serde_yaml::from_str(r#""sudo apt update""#).unwrap(),
                        uninstall: StringOrVec::default(),
                        shell: None,
                        env: HashMap::new(),
                    }),
                ],
                os: Default::default(),
                parallel: false,
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                auto_update: Some(AutoUpdateConfig {
                    at: Some("07:30".into()),
                    cron: None,
                }),
            },
        );
        let config = make_config(tasks);
        let names = vec!["priv".to_string()];
        let (demoted, warnings) = demote_config_for_schedule(&config, &names);

        assert!(!warnings.is_empty());
        let task = &demoted.tasks["priv"];
        match &task.commands[0] {
            CommandEntry::Copy(a) => assert!(!a.sudo),
            _ => panic!("expected copy"),
        }
        match &task.commands[1] {
            CommandEntry::Symlink(a) => assert!(!a.sudo),
            _ => panic!("expected symlink"),
        }
        match &task.commands[2] {
            CommandEntry::Run(a) => {
                assert_eq!(
                    a.commands_for_mode(crate::engine::mode::Mode::Update),
                    &["apt update"]
                );
            }
            _ => panic!("expected run"),
        }
        // Original config unchanged
        match &config.tasks["priv"].commands[0] {
            CommandEntry::Copy(a) => assert!(a.sudo),
            _ => panic!("expected copy"),
        }
    }

    #[test]
    fn demote_skips_tasks_not_in_selection() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "other".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Copy(CopyArgs {
                    src: "/a".into(),
                    target: "/b".into(),
                    ignore: vec![],
                    sudo: true,
                })],
                os: Default::default(),
                parallel: false,
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                auto_update: None,
            },
        );
        let config = make_config(tasks);
        let (demoted, warnings) = demote_config_for_schedule(&config, &[]);
        assert!(warnings.is_empty());
        match &demoted.tasks["other"].commands[0] {
            CommandEntry::Copy(a) => assert!(a.sudo),
            _ => panic!("expected copy"),
        }
    }
}
