//! `schedule run --key` — update installed tasks for a schedule key.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::history::History;
use crate::config::types::AppConfig;
use crate::engine::mode::Mode;
use crate::engine::runner::TaskRunner;
use crate::engine::sink::{NullSink, SharedSink};
use crate::error::{Error, Result};
use crate::schedule::group;
use crate::schedule::key::ScheduleKey;
use crate::schedule::notices::NoticeStore;

/// Run update mode for installed tasks matching `key`.
pub async fn run_key(
    config: AppConfig,
    config_dir: PathBuf,
    key: &ScheduleKey,
    temp_dir: &Path,
) -> Result<RunReport> {
    let candidates = group::tasks_for_key(&config, key)?;
    if candidates.is_empty() {
        return Err(Error::ScheduleError(format!(
            "no tasks declare auto_update for key {}",
            key.as_str()
        )));
    }

    let history = History::load(temp_dir).unwrap_or_default();
    let mut to_run: Vec<String> = candidates
        .into_iter()
        .filter(|name| history.is_installed(name))
        .collect();

    if to_run.is_empty() {
        append_log(
            temp_dir,
            &format!(
                "schedule run {}: no installed tasks (candidates skipped)\n",
                key.as_str()
            ),
        )?;
        return Ok(RunReport {
            key: key.as_str().to_string(),
            updated: vec![],
            failed: vec![],
            skipped_not_installed: true,
        });
    }

    to_run.sort();
    append_log(
        temp_dir,
        &format!(
            "schedule run {}: updating {}\n",
            key.as_str(),
            to_run.join(", ")
        ),
    )?;

    let (config, demote_warnings) =
        crate::schedule::demote_sudo::demote_config_for_schedule(&config, &to_run);
    for line in &demote_warnings {
        append_log(temp_dir, &format!("  warn: {line}\n"))?;
    }

    let events: SharedSink = NullSink::shared();
    let runner = TaskRunner::new(config, Mode::Update, events).with_config_dir(config_dir);

    // Run one-by-one so we can attribute success/failure per task for notices.
    let mut updated = Vec::new();
    let mut failed = Vec::new();
    for name in &to_run {
        match runner.run_single_task(name, true).await {
            Ok(()) => {
                updated.push(name.clone());
                append_log(temp_dir, &format!("  ok: {name}\n"))?;
            }
            Err(e) => {
                failed.push(name.clone());
                append_log(temp_dir, &format!("  fail: {name}: {e}\n"))?;
            }
        }
    }

    if !updated.is_empty() || !failed.is_empty() {
        let mut notices = NoticeStore::load(temp_dir)?;
        notices.append(key.as_str(), updated.clone(), failed.clone());
        notices.save(temp_dir)?;
    }

    Ok(RunReport {
        key: key.as_str().to_string(),
        updated,
        failed,
        skipped_not_installed: false,
    })
}

#[derive(Debug)]
pub struct RunReport {
    pub key: String,
    pub updated: Vec<String>,
    pub failed: Vec<String>,
    pub skipped_not_installed: bool,
}

fn append_log(temp_dir: &Path, line: &str) -> Result<()> {
    std::fs::create_dir_all(temp_dir)?;
    let path = temp_dir.join("schedule.log");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

/// Parse `--key` CLI value into [`ScheduleKey`].
pub fn parse_key_arg(raw: &str) -> Result<ScheduleKey> {
    let s = raw.trim();
    if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
        let hour: u32 = s[0..2]
            .parse()
            .map_err(|_| Error::ScheduleError(format!("invalid schedule key '{raw}'")))?;
        let minute: u32 = s[2..4]
            .parse()
            .map_err(|_| Error::ScheduleError(format!("invalid schedule key '{raw}'")))?;
        return ScheduleKey::from_hour_minute(hour, minute);
    }
    // Allow HH:MM as convenience
    let cfg = crate::config::types::AutoUpdateConfig {
        at: Some(s.to_string()),
        cron: None,
    };
    ScheduleKey::parse_auto_update(&cfg)
}
