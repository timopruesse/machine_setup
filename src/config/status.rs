//! Task status — join Config document Tasks with History for `list` / `doctor`.

use chrono::{DateTime, Utc};
use std::path::Path;

use super::history::{History, TaskHistory};
use super::os::OsFilter;
use super::types::{AppConfig, TaskConfig};
use super::validate::{self, ValidationIssue};

/// One row of Task status for display.
#[derive(Debug, Clone)]
pub struct TaskStatusRow<'a> {
    pub name: &'a str,
    pub task: &'a TaskConfig,
    pub installed: bool,
    pub os_applies: bool,
    pub history: Option<&'a TaskHistory>,
}

/// Full doctor report: status rows, validate issues, orphan History keys.
#[derive(Debug)]
pub struct DoctorReport<'a> {
    pub rows: Vec<TaskStatusRow<'a>>,
    pub issues: Vec<ValidationIssue>,
    pub orphans: Vec<String>,
}

impl DoctorReport<'_> {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| matches!(i.severity, validate::Severity::Error))
    }
}

/// Build status rows in Config document Task order.
pub fn rows<'a>(config: &'a AppConfig, history: &'a History) -> Vec<TaskStatusRow<'a>> {
    config
        .tasks
        .iter()
        .map(|(name, task)| {
            let hist = history.tasks.get(name);
            TaskStatusRow {
                name,
                task,
                installed: history.is_installed(name),
                os_applies: task.os.matches_current(),
                history: hist,
            }
        })
        .collect()
}

/// History task names that are not defined in the Config document.
pub fn orphan_history_names(config: &AppConfig, history: &History) -> Vec<String> {
    let mut names: Vec<String> = history
        .tasks
        .keys()
        .filter(|name| !config.tasks.contains_key(*name))
        .cloned()
        .collect();
    names.sort();
    names
}

/// Remove orphan History entries. Returns the names removed.
pub fn prune_orphans(history: &mut History, config: &AppConfig) -> Vec<String> {
    let orphans = orphan_history_names(config, history);
    for name in &orphans {
        history.tasks.remove(name);
    }
    orphans
}

/// Build a doctor report (does not mutate History).
pub fn doctor<'a>(
    config: &'a AppConfig,
    history: &'a History,
    config_dir: &Path,
) -> DoctorReport<'a> {
    DoctorReport {
        rows: rows(config, history),
        issues: validate::validate_config(config, config_dir),
        orphans: orphan_history_names(config, history),
    }
}

/// Format an optional timestamp for list output.
pub fn format_ts(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "-".into())
}

/// Human-readable OS filter label (same idea as previous `list`).
pub fn os_label(os: &OsFilter) -> String {
    match os {
        OsFilter::All => "all".to_string(),
        OsFilter::Single(o) => o.to_string(),
        OsFilter::Multiple(oses) => oses
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use indexmap::IndexMap;

    fn empty_task() -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os: OsFilter::All,
            parallel: false,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: Default::default(),
            retry: 0,
            retry_delay_secs: 1,
            auto_update: None,
        }
    }

    fn config_with(names: &[&str]) -> AppConfig {
        let mut tasks = IndexMap::new();
        for name in names {
            tasks.insert((*name).into(), empty_task());
        }
        AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".into(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
            check_for_updates: true,
        }
    }

    #[test]
    fn rows_mark_installed_from_history() {
        let config = config_with(&["a", "b"]);
        let mut history = History::default();
        history.mark_installed("a");

        let rows = rows(&config, &history);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].installed);
        assert!(!rows[1].installed);
    }

    #[test]
    fn orphan_history_names_lists_missing_tasks() {
        let config = config_with(&["a"]);
        let mut history = History::default();
        history.mark_installed("a");
        history.mark_installed("gone");
        assert_eq!(orphan_history_names(&config, &history), vec!["gone"]);
    }

    #[test]
    fn prune_orphans_removes_only_orphans() {
        let config = config_with(&["a"]);
        let mut history = History::default();
        history.mark_installed("a");
        history.mark_installed("gone");
        let removed = prune_orphans(&mut history, &config);
        assert_eq!(removed, vec!["gone"]);
        assert!(history.tasks.contains_key("a"));
        assert!(!history.tasks.contains_key("gone"));
    }

    #[test]
    fn doctor_includes_empty_task_warning() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_with(&["empty"]);
        let history = History::default();
        let report = doctor(&config, &history, dir.path());
        assert!(report
            .issues
            .iter()
            .any(|i| i.task_name == "empty" && i.message.contains("no commands")));
        assert!(report.orphans.is_empty());
    }
}
