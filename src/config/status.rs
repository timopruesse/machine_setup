//! Task status — join Config document Tasks with History for `list`.

use chrono::{DateTime, Utc};

use super::history::{History, TaskHistory};
use super::os::OsFilter;
use super::types::{AppConfig, TaskConfig};

/// One row of Task status for display.
#[derive(Debug, Clone)]
pub struct TaskStatusRow<'a> {
    pub name: &'a str,
    pub task: &'a TaskConfig,
    pub installed: bool,
    pub os_applies: bool,
    pub history: Option<&'a TaskHistory>,
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
        }
    }

    #[test]
    fn rows_mark_installed_from_history() {
        let mut tasks = IndexMap::new();
        tasks.insert("a".into(), empty_task());
        tasks.insert("b".into(), empty_task());
        let config = AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".into(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
        };
        let mut history = History::default();
        history.mark_installed("a");

        let rows = rows(&config, &history);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].installed);
        assert!(!rows[1].installed);
    }
}
