use std::path::Path;

use super::graph::TaskGraph;
use super::types::{AppConfig, Condition};
use crate::engine::commands::catalog::{self, KindSeverity};

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

/// Validate depends_on references exist and detect cycles, using the shared
/// [`TaskGraph`] so ordering and validation agree on the same logic.
fn validate_dependencies(config: &AppConfig, issues: &mut Vec<ValidationIssue>) {
    let graph = TaskGraph::new(&config.tasks);

    // Report each broken edge.
    for (name, dep) in graph.missing_dependencies() {
        issues.push(ValidationIssue {
            task_name: name,
            message: format!("depends_on references unknown task: '{dep}'"),
            severity: Severity::Error,
        });
    }

    // Report one cycle, if any.
    if let Some(cycle) = graph.find_cycle() {
        issues.push(ValidationIssue {
            task_name: cycle[0].clone(),
            message: format!("Cyclic dependency detected: {}", cycle.join(" -> ")),
            severity: Severity::Error,
        });
    }
}

pub fn validate_config(config: &AppConfig, config_dir: &Path) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // Validate depends_on references and detect cycles
    validate_dependencies(config, &mut issues);

    for (name, task) in &config.tasks {
        validate_conditions(name, &task.only_if, "only_if", &mut issues);
        validate_conditions(name, &task.skip_if, "skip_if", &mut issues);

        if task.commands.is_empty() {
            issues.push(ValidationIssue {
                task_name: name.clone(),
                message: "Task has no commands".to_string(),
                severity: Severity::Warning,
            });
        }

        for cmd in &task.commands {
            for kind_issue in catalog::validate_entry(cmd, config_dir) {
                issues.push(ValidationIssue {
                    task_name: name.clone(),
                    message: kind_issue.message,
                    severity: match kind_issue.severity {
                        KindSeverity::Error => Severity::Error,
                        KindSeverity::Warning => Severity::Warning,
                    },
                });
            }
        }

        if let Some(auto) = &task.auto_update {
            if let Err(msg) = crate::schedule::ScheduleKey::parse_auto_update(auto) {
                issues.push(ValidationIssue {
                    task_name: name.clone(),
                    message: msg,
                    severity: Severity::Error,
                });
            } else if task.commands.iter().any(catalog::entry_requires_sudo) {
                issues.push(ValidationIssue {
                    task_name: name.clone(),
                    message: "auto_update task requires sudo; schedule run will demote privileges and may fail without a password".to_string(),
                    severity: Severity::Warning,
                });
            }
        }
    }

    issues
}

fn validate_conditions(
    task_name: &str,
    conditions: &super::types::Conditions,
    field: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for cond in conditions.iter() {
        match cond {
            Condition::Path(path) if path.trim().is_empty() => {
                issues.push(ValidationIssue {
                    task_name: task_name.to_string(),
                    message: format!("{field} contains an empty path"),
                    severity: Severity::Warning,
                });
            }
            Condition::Env(var) if var.trim().is_empty() => {
                issues.push(ValidationIssue {
                    task_name: task_name.to_string(),
                    message: format!("{field} contains an empty env var name"),
                    severity: Severity::Warning,
                });
            }
            Condition::Command(cmd) if cmd.trim().is_empty() => {
                issues.push(ValidationIssue {
                    task_name: task_name.to_string(),
                    message: format!("{field} contains an empty command"),
                    severity: Severity::Warning,
                });
            }
            Condition::Mode(modes) if modes.is_empty() => {
                issues.push(ValidationIssue {
                    task_name: task_name.to_string(),
                    message: format!("{field} contains an empty mode list"),
                    severity: Severity::Warning,
                });
            }
            _ => {}
        }
    }
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
            check_for_updates: true,
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
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                retry_delay_secs: 1,
                auto_update: None,
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
                    quiet: false,
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
                    force: false,
                    with_deps: false,
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
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                retry_delay_secs: 1,
                auto_update: None,
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
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                retry_delay_secs: 1,
                auto_update: None,
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, dir.path());
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_auto_update_non_daily_cron() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "bun".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Run(RunArgs {
                    commands: StringOrVec::default(),
                    install: StringOrVec::default(),
                    update: StringOrVec::default(),
                    uninstall: StringOrVec::default(),
                    shell: None,
                    env: HashMap::new(),
                    quiet: false,
                })],
                os: Default::default(),
                parallel: false,
                only_if: Default::default(),
                skip_if: Default::default(),
                depends_on: Default::default(),
                retry: 0,
                retry_delay_secs: 1,
                auto_update: Some(AutoUpdateConfig {
                    at: None,
                    cron: Some("0 7 * * 1".into()),
                }),
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues.iter().any(|i| {
            i.task_name == "bun"
                && i.message.contains("daily")
                && matches!(i.severity, Severity::Error)
        }));
    }

    #[test]
    fn test_validate_auto_update_sudo_is_warning() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "priv".to_string(),
            TaskConfig {
                commands: vec![CommandEntry::Copy(CopyArgs {
                    src: "/nonexistent/source".to_string(),
                    target: "/tmp/target".to_string(),
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
                auto_update: Some(AutoUpdateConfig {
                    at: Some("07:30".into()),
                    cron: None,
                }),
            },
        );
        let config = make_config(tasks);
        let issues = validate_config(&config, Path::new("."));
        assert!(issues.iter().any(|i| {
            i.task_name == "priv"
                && i.message.contains("demote")
                && matches!(i.severity, Severity::Warning)
        }));
        assert!(!issues.iter().any(|i| {
            i.task_name == "priv"
                && i.message.contains("demote")
                && matches!(i.severity, Severity::Error)
        }));
    }
}
