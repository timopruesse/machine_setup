use crate::config::history::History;
use crate::config::status::{self, format_ts, os_label};
use crate::config::types::AppConfig;

use super::model::{CatalogItem, CatalogStatus, DetailSection};

pub fn list_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    status::rows(config, history)
        .into_iter()
        .map(row_to_item)
        .collect()
}

pub fn select_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    list_items(config, history)
}

fn row_to_item(row: status::TaskStatusRow<'_>) -> CatalogItem {
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
}
