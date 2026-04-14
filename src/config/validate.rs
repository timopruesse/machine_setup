use std::path::Path;

use super::types::{AppConfig, CommandEntry};
use crate::utils::shell::validate_env_key;

#[derive(Debug)]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
        }
    }
}

#[derive(Debug)]
pub struct ValidationIssue {
    pub task_name: String,
    pub message: String,
    pub severity: Severity,
}

pub fn validate_config(config: &AppConfig, config_dir: &Path) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for (name, task) in &config.tasks {
        if task.commands.is_empty() {
            issues.push(ValidationIssue {
                task_name: name.clone(),
                message: "Task has no commands".to_string(),
                severity: Severity::Warning,
            });
        }

        for cmd in &task.commands {
            match cmd {
                CommandEntry::Run(args) => {
                    if args.all_command_strings().is_empty() {
                        issues.push(ValidationIssue {
                            task_name: name.clone(),
                            message: format!("Run command has no commands defined: {cmd}"),
                            severity: Severity::Warning,
                        });
                    }
                    for key in args.env.keys() {
                        if !validate_env_key(key) {
                            issues.push(ValidationIssue {
                                task_name: name.clone(),
                                message: format!("Invalid environment variable name: {key:?}"),
                                severity: Severity::Error,
                            });
                        }
                    }
                }
                CommandEntry::Copy(args) => {
                    let src = crate::utils::path::expand_path(&args.src, Some(config_dir));
                    if !src.exists() {
                        issues.push(ValidationIssue {
                            task_name: name.clone(),
                            message: format!("Copy source does not exist: {}", src.display()),
                            severity: Severity::Warning,
                        });
                    }
                }
                CommandEntry::Symlink(args) => {
                    let src = crate::utils::path::expand_path(&args.src, Some(config_dir));
                    if !src.exists() {
                        issues.push(ValidationIssue {
                            task_name: name.clone(),
                            message: format!("Symlink source does not exist: {}", src.display()),
                            severity: Severity::Warning,
                        });
                    }
                }
                CommandEntry::MachineSetup(args) => {
                    if !crate::config::is_url(&args.config) {
                        let path = crate::utils::path::expand_path(&args.config, Some(config_dir));
                        // Check with common extensions too
                        let exists = path.exists()
                            || path.with_extension("yaml").exists()
                            || path.with_extension("yml").exists()
                            || path.with_extension("json").exists();
                        if !exists {
                            issues.push(ValidationIssue {
                                task_name: name.clone(),
                                message: format!("Sub-config not found: {}", path.display()),
                                severity: Severity::Error,
                            });
                        }
                    }
                }
                CommandEntry::Clone(_) => {}
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use indexmap::IndexMap;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_config(tasks: IndexMap<String, TaskConfig>) -> AppConfig {
        AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".to_string(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
        }
    }

    #[test]
    fn test_validate_empty_task() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "empty".to_string(),
            TaskConfig {
                commands: vec![],
                os: Default::default(),
                parallel: false,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues.iter().any(|i| i.task_name == "empty"
            && i.message.contains("no commands")
            && matches!(i.severity, Severity::Warning)));
    }

    #[test]
    fn test_validate_invalid_env_key() {
        let mut env = HashMap::new();
        env.insert("BAD-KEY".to_string(), "value".to_string());
        let mut tasks = IndexMap::new();
        tasks.insert(
            "test".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Run(RunArgs {
                    commands: StringOrVec::default(),
                    install: StringOrVec::default(),
                    update: StringOrVec::default(),
                    uninstall: StringOrVec::default(),
                    shell: None,
                    env,
                })],
                os: Default::default(),
                parallel: false,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues
            .iter()
            .any(|i| i.message.contains("BAD-KEY") && matches!(i.severity, Severity::Error)));
    }

    #[test]
    fn test_validate_missing_sub_config() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "sub".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::MachineSetup(MachineSetupArgs {
                    config: "/nonexistent/config".to_string(),
                    task: None,
                })],
                os: Default::default(),
                parallel: false,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues
            .iter()
            .any(|i| i.message.contains("Sub-config not found")
                && matches!(i.severity, Severity::Error)));
    }

    #[test]
    fn test_validate_missing_copy_source() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "copy_task".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Copy(CopyArgs {
                    src: "/nonexistent/source".to_string(),
                    target: "/tmp/target".to_string(),
                    ignore: vec![],
                    sudo: false,
                })],
                os: Default::default(),
                parallel: false,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues
            .iter()
            .any(|i| i.message.contains("Copy source") && matches!(i.severity, Severity::Warning)));
    }

    #[test]
    fn test_validate_valid_config() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source_file");
        std::fs::write(&src, "content").unwrap();

        let mut tasks = IndexMap::new();
        tasks.insert(
            "valid".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Copy(CopyArgs {
                    src: src.to_string_lossy().to_string(),
                    target: "/tmp/target".to_string(),
                    ignore: vec![],
                    sudo: false,
                })],
                os: Default::default(),
                parallel: false,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, dir.path());
        assert!(issues.is_empty());
    }
}
