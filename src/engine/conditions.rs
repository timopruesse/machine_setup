use std::path::Path;

use crate::config::history::History;
use crate::config::types::{Condition, Shell, TaskConfig};
use crate::engine::mode::Mode;
use crate::utils::path::expand_path;
use crate::utils::shell::shell_binary;

/// Decide whether a task should be skipped. Returns `Some(reason)` when the
/// task must not run (OS filter, conditions, or install History).
pub fn evaluate_skip(
    task: &TaskConfig,
    name: &str,
    mode: Mode,
    force: bool,
    history: &History,
    config_dir: &Path,
    default_shell: &Shell,
) -> Option<String> {
    if !task.os.matches_current() {
        return Some("OS mismatch".to_string());
    }

    for cond in task.only_if.iter() {
        if let Some(reason) = evaluate_only_if(cond, mode, config_dir, default_shell) {
            return Some(reason);
        }
    }

    for cond in task.skip_if.iter() {
        if let Some(reason) = evaluate_skip_if(cond, mode, config_dir, default_shell) {
            return Some(reason);
        }
    }

    if mode == Mode::Install && !force && history.is_installed(name) {
        return Some("Already installed (use --force to reinstall)".to_string());
    }

    None
}

fn evaluate_only_if(
    cond: &Condition,
    mode: Mode,
    config_dir: &Path,
    default_shell: &Shell,
) -> Option<String> {
    match cond {
        Condition::Path(path_str) => {
            let path = expand_path(path_str, Some(config_dir));
            if !path.exists() {
                Some(format!("Condition not met: '{path_str}' does not exist"))
            } else {
                None
            }
        }
        Condition::Env(var) => match std::env::var(var) {
            Ok(v) if !v.is_empty() => None,
            _ => Some(format!("Condition not met: env '{var}' is unset or empty")),
        },
        Condition::Command(command) => {
            if command_succeeds(command, default_shell) {
                None
            } else {
                Some(format!("Condition not met: command failed: '{command}'"))
            }
        }
        Condition::Mode(modes) => {
            if modes.contains(&mode) {
                None
            } else {
                Some(format!(
                    "Condition not met: mode '{mode}' not in [{}]",
                    modes
                        .iter()
                        .map(Mode::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }
}

fn evaluate_skip_if(
    cond: &Condition,
    mode: Mode,
    config_dir: &Path,
    default_shell: &Shell,
) -> Option<String> {
    match cond {
        Condition::Path(path_str) => {
            let path = expand_path(path_str, Some(config_dir));
            if path.exists() {
                Some(format!("Skipped: '{path_str}' exists"))
            } else {
                None
            }
        }
        Condition::Env(var) => match std::env::var(var) {
            Ok(v) if !v.is_empty() => Some(format!("Skipped: env '{var}' is set")),
            _ => None,
        },
        Condition::Command(command) => {
            if command_succeeds(command, default_shell) {
                Some(format!("Skipped: command succeeded: '{command}'"))
            } else {
                None
            }
        }
        Condition::Mode(modes) => {
            if modes.contains(&mode) {
                Some(format!("Skipped: mode is '{mode}'"))
            } else {
                None
            }
        }
    }
}

fn command_succeeds(command: &str, shell: &Shell) -> bool {
    let binary = shell_binary(shell);
    match shell {
        Shell::Bash | Shell::Zsh => std::process::Command::new(binary)
            .arg("-c")
            .arg(command)
            .status()
            .map(|s| s.success())
            .unwrap_or(false),
        Shell::PowerShell => {
            if cfg!(windows) {
                std::process::Command::new(binary)
                    .args(["-NoProfile", "-Command", command])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            } else {
                std::process::Command::new(binary)
                    .args(["-NoProfile", "-Command", command])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::Conditions;
    use std::env;
    use tempfile::tempdir;

    fn task_with(only_if: Conditions, skip_if: Conditions) -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os: Default::default(),
            parallel: false,
            only_if,
            skip_if,
            depends_on: vec![],
            retry: 0,
            retry_delay_secs: 1,
            auto_update: None,
        }
    }

    #[test]
    fn only_if_path_missing_skips() {
        let dir = tempdir().unwrap();
        let task = task_with(
            vec![Condition::Path("/nonexistent/path".into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn only_if_path_exists_runs() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("marker");
        std::fs::write(&marker, "").unwrap();
        let task = task_with(
            vec![Condition::Path(marker.to_string_lossy().into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_none());
    }

    #[test]
    fn skip_if_path_exists_skips() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("marker");
        std::fs::write(&marker, "").unwrap();
        let task = task_with(
            Conditions::default(),
            vec![Condition::Path(marker.to_string_lossy().into())].into(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn only_if_env_set_runs() {
        let dir = tempdir().unwrap();
        env::set_var("MACHINE_SETUP_TEST_COND_VAR", "yes");
        let task = task_with(
            vec![Condition::Env("MACHINE_SETUP_TEST_COND_VAR".into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_none());
        env::remove_var("MACHINE_SETUP_TEST_COND_VAR");
    }

    #[test]
    fn only_if_env_unset_skips() {
        let dir = tempdir().unwrap();
        env::remove_var("MACHINE_SETUP_TEST_COND_UNSET");
        let task = task_with(
            vec![Condition::Env("MACHINE_SETUP_TEST_COND_UNSET".into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn only_if_mode_matches_runs() {
        let dir = tempdir().unwrap();
        let task = task_with(
            vec![Condition::Mode(vec![Mode::Install, Mode::Update])].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_none());
    }

    #[test]
    fn only_if_mode_mismatch_skips() {
        let dir = tempdir().unwrap();
        let task = task_with(
            vec![Condition::Mode(vec![Mode::Update])].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn skip_if_mode_matches_skips() {
        let dir = tempdir().unwrap();
        let task = task_with(
            Conditions::default(),
            vec![Condition::Mode(vec![Mode::Uninstall])].into(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Uninstall,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn only_if_command_true_runs() {
        let dir = tempdir().unwrap();
        let task = task_with(
            vec![Condition::Command("true".into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_none());
    }

    #[test]
    fn only_if_command_false_skips() {
        let dir = tempdir().unwrap();
        let task = task_with(
            vec![Condition::Command("false".into())].into(),
            Conditions::default(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }

    #[test]
    fn skip_if_command_true_skips() {
        let dir = tempdir().unwrap();
        let task = task_with(
            Conditions::default(),
            vec![Condition::Command("true".into())].into(),
        );
        assert!(evaluate_skip(
            &task,
            "t",
            Mode::Install,
            false,
            &History::default(),
            dir.path(),
            &Shell::Bash,
        )
        .is_some());
    }
}
