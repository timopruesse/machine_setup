//! Demote sudo for non-interactive `schedule run` (no TTY / password prompt).

use std::collections::HashSet;

use crate::config::types::AppConfig;
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
            catalog::demote_entry_for_unattended(name, entry, &mut warnings);
        }
    }

    (demoted, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{
        AutoUpdateConfig, CommandEntry, CopyArgs, RunArgs, StringOrVec, SymlinkArgs, TaskConfig,
    };
    use indexmap::IndexMap;
    use std::collections::HashMap;

    fn make_config(tasks: IndexMap<String, TaskConfig>) -> AppConfig {
        AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".to_string(),
            default_shell: crate::config::types::Shell::Bash,
            parallel: false,
            num_threads: None,
            check_for_updates: true,
        }
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
                        quiet: false,
                    }),
                ],
                os: Default::default(),
                parallel: false,
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                retry_delay_secs: 1,
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
                retry_delay_secs: 1,
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
