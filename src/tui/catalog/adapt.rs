use crate::config::history::History;
use crate::config::status::{self, format_ts, os_label, DoctorReport};
use crate::config::types::AppConfig;
use crate::config::validate::{Severity, ValidationIssue};

use super::model::{CatalogItem, CatalogStatus, DetailSection};

pub fn list_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    status::rows(config, history)
        .into_iter()
        .map(|row| row_to_item(&row, &[]))
        .collect()
}

pub fn select_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    list_items(config, history)
}

/// Catalog rows for `doctor`: list status plus per-task validation detail/badges.
pub fn doctor_items(report: &DoctorReport<'_>) -> Vec<CatalogItem> {
    report
        .rows
        .iter()
        .map(|row| {
            let task_issues: Vec<&ValidationIssue> = report
                .issues
                .iter()
                .filter(|issue| issue.task_name == row.name)
                .collect();
            row_to_item(row, &task_issues)
        })
        .collect()
}

/// Summary banner lines for the doctor TUI.
pub fn doctor_banner(report: &DoctorReport<'_>, fix: bool) -> Vec<String> {
    let mut lines = Vec::new();

    let errors = report
        .issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Error))
        .count();
    let warnings = report
        .issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Warning))
        .count();

    if report.issues.is_empty() {
        lines.push("Validation: config is valid.".into());
    } else {
        lines.push(format!(
            "Validation: {errors} error{}, {warnings} warning{}",
            if errors == 1 { "" } else { "s" },
            if warnings == 1 { "" } else { "s" },
        ));
    }

    if report.orphans.is_empty() {
        lines.push("History orphans: none.".into());
    } else {
        lines.push(format!(
            "History orphans: {} ({})",
            report.orphans.len(),
            report.orphans.join(", ")
        ));
        if !fix {
            lines.push("Hint: re-run with `doctor --fix` to remove orphan History entries.".into());
        }
    }

    lines
}

fn row_to_item(row: &status::TaskStatusRow<'_>, issues: &[&ValidationIssue]) -> CatalogItem {
    let status = if !row.os_applies {
        CatalogStatus::SkippedOs
    } else if row.installed {
        CatalogStatus::Installed
    } else {
        CatalogStatus::NotInstalled
    };

    let mut badges = Vec::new();
    if row.task.parallel {
        badges.push("parallel".into());
    }
    if !row.os_applies {
        badges.push("os skip".into());
    }
    if issues.iter().any(|i| matches!(i.severity, Severity::Error)) {
        badges.push("error".into());
    } else if issues
        .iter()
        .any(|i| matches!(i.severity, Severity::Warning))
    {
        badges.push("warn".into());
    }

    let (installed_at, updated_at) = match row.history {
        Some(h) => (format_ts(h.installed_at), format_ts(h.updated_at)),
        None => ("-".into(), "-".into()),
    };

    let os = os_label(&row.task.os);

    let mut detail = vec![
        DetailSection {
            title: "Meta".into(),
            lines: vec![
                format!("OS: {os}"),
                format!("Installed: {}", if row.installed { "yes" } else { "no" }),
                match &row.task.auto_update {
                    Some(au) => match crate::schedule::ScheduleKey::parse_auto_update(au) {
                        Ok(key) => format!("auto_update: daily {key}"),
                        Err(e) => format!("auto_update: invalid ({e})"),
                    },
                    None => "auto_update: —".into(),
                },
            ],
        },
        DetailSection {
            title: "History".into(),
            lines: vec![
                format!("installed_at: {installed_at}"),
                format!("updated_at: {updated_at}"),
            ],
        },
    ];

    let cmd_lines: Vec<String> = row.task.commands.iter().map(|c| format!("- {c}")).collect();
    detail.push(DetailSection {
        title: "Commands".into(),
        lines: if cmd_lines.is_empty() {
            vec!["(none)".into()]
        } else {
            cmd_lines
        },
    });

    if !issues.is_empty() {
        detail.push(DetailSection {
            title: "Validation".into(),
            lines: issues
                .iter()
                .map(|i| format!("[{}] {}", i.severity, i.message))
                .collect(),
        });
    }

    CatalogItem {
        id: row.name.to_string(),
        title: row.name.to_string(),
        status,
        os_label: os,
        installed_at,
        updated_at,
        badges,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::os::{Os, OsFilter};
    use crate::config::types::*;
    use indexmap::IndexMap;

    fn empty_task() -> TaskConfig {
        task_with_os(OsFilter::All)
    }

    fn task_with_os(os: OsFilter) -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os,
            parallel: false,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: Default::default(),
            retry: 0,
            auto_update: None,
        }
    }

    fn config_with(names: &[&str]) -> AppConfig {
        config_with_tasks(names.iter().map(|name| (*name, empty_task())).collect())
    }

    fn config_with_tasks(tasks: Vec<(&str, TaskConfig)>) -> AppConfig {
        let mut map = IndexMap::new();
        for (name, task) in tasks {
            map.insert(name.into(), task);
        }
        AppConfig {
            tasks: map,
            temp_dir: "~/.machine_setup".into(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
        }
    }

    /// Pick an OS that does not match the current host.
    fn non_current_os() -> Os {
        match Os::current() {
            Some(Os::Windows) => Os::Linux,
            _ => Os::Windows,
        }
    }

    fn history_section(item: &CatalogItem) -> Option<&DetailSection> {
        item.detail.iter().find(|s| s.title == "History")
    }

    #[test]
    fn list_items_marks_installed_and_not_installed() {
        let config = config_with(&["installed-task", "pending-task"]);
        let mut history = History::default();
        history.mark_installed("installed-task");

        let items = list_items(&config, &history);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "installed-task");
        assert_eq!(items[0].status, CatalogStatus::Installed);
        assert_eq!(items[1].id, "pending-task");
        assert_eq!(items[1].status, CatalogStatus::NotInstalled);
    }

    #[test]
    fn list_items_includes_history_detail_section() {
        let config = config_with(&["a"]);
        let mut history = History::default();
        history.mark_installed("a");

        let items = list_items(&config, &history);
        let item = &items[0];
        assert_eq!(item.os_label, "all");
        assert_ne!(item.installed_at, "-");
        assert_eq!(item.updated_at, "-");

        let section = history_section(item).expect("History detail section");
        assert!(section.lines.iter().any(|l| l.starts_with("installed_at:")));
        assert!(section.lines.iter().any(|l| l.starts_with("updated_at:")));
    }

    #[test]
    fn list_items_marks_skipped_os_with_badge() {
        let foreign = non_current_os();
        let os_label = foreign.to_string();
        let config = config_with_tasks(vec![(
            "windows-only",
            task_with_os(OsFilter::Single(foreign)),
        )]);
        let history = History::default();

        let items = list_items(&config, &history);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, CatalogStatus::SkippedOs);
        assert!(items[0].badges.contains(&"os skip".to_string()));
        assert_eq!(items[0].os_label, os_label);
    }

    #[test]
    fn select_items_delegates_to_list_items() {
        let config = config_with(&["a", "b"]);
        let history = History::default();

        assert_eq!(
            select_items(&config, &history),
            list_items(&config, &history)
        );
    }

    #[test]
    fn doctor_items_attaches_validation_section_and_badges() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_with(&["empty"]);
        let history = History::default();
        let report = status::doctor(&config, &history, dir.path());

        let items = doctor_items(&report);
        assert_eq!(items.len(), 1);
        assert!(
            items[0].badges.contains(&"warn".to_string())
                || items[0].badges.contains(&"error".to_string())
        );
        let section = items[0]
            .detail
            .iter()
            .find(|s| s.title == "Validation")
            .expect("Validation detail");
        assert!(!section.lines.is_empty());
    }

    #[test]
    fn doctor_banner_mentions_valid_and_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_with(&["a"]);
        let mut history = History::default();
        history.mark_installed("a");
        history.mark_installed("gone");
        let report = status::doctor(&config, &history, dir.path());

        let banner = doctor_banner(&report, false);
        assert!(banner.iter().any(|l| l.contains("orphan")));
        assert!(banner.iter().any(|l| l.contains("doctor --fix")));

        let banner_fix = doctor_banner(&report, true);
        assert!(banner_fix.iter().all(|l| !l.contains("doctor --fix")));
    }
}
