//! Group Config tasks by normalized schedule key.

use std::collections::BTreeMap;

use crate::config::types::AppConfig;
use crate::error::{Error, Result};
use crate::schedule::key::ScheduleKey;

/// Map each valid daily key to task names (Config document order preserved per key).
pub fn group_keys(config: &AppConfig) -> Result<BTreeMap<ScheduleKey, Vec<String>>> {
    let mut map: BTreeMap<ScheduleKey, Vec<String>> = BTreeMap::new();
    for (name, task) in &config.tasks {
        let Some(auto) = &task.auto_update else {
            continue;
        };
        let key = ScheduleKey::parse_auto_update(auto)
            .map_err(|e| Error::ScheduleError(format!("task `{name}`: {e}")))?;
        map.entry(key).or_default().push(name.clone());
    }
    Ok(map)
}

/// Task names whose auto_update normalizes to `key`.
pub fn tasks_for_key(config: &AppConfig, key: &ScheduleKey) -> Result<Vec<String>> {
    let grouped = group_keys(config)?;
    Ok(grouped.get(key).cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use indexmap::IndexMap;
    use std::sync::Arc;

    fn task_with(auto: Option<AutoUpdateConfig>) -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os: Default::default(),
            parallel: false,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: vec![],
            retry: 0,
            retry_delay_secs: 1,
            auto_update: auto,
        }
    }

    #[test]
    fn bundles_same_time() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "bun".into(),
            Arc::new(task_with(Some(AutoUpdateConfig {
                at: Some("07:30".into()),
                cron: None,
            }))),
        );
        tasks.insert(
            "node".into(),
            Arc::new(task_with(Some(AutoUpdateConfig {
                at: None,
                cron: Some("30 7 * * *".into()),
            }))),
        );
        let config = AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".into(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
            check_for_updates: true,
        };
        let g = group_keys(&config).unwrap();
        assert_eq!(g.len(), 1);
        let names = g.values().next().unwrap();
        assert_eq!(names, &vec!["bun".to_string(), "node".to_string()]);
    }
}
