//! Persisted schedule notices for the shell hook.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleNotice {
    pub key: String,
    pub updated: Vec<String>,
    pub failed: Vec<String>,
    pub at: DateTime<Utc>,
    #[serde(default)]
    pub seen: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoticeStore {
    pub notices: Vec<ScheduleNotice>,
}

impl NoticeStore {
    fn path(temp_dir: &Path) -> PathBuf {
        temp_dir.join("schedule_notices.json")
    }

    pub fn load(temp_dir: &Path) -> Result<Self> {
        let path = Self::path(temp_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|e| Error::ScheduleError(e.to_string()))
    }

    pub fn save(&self, temp_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(temp_dir)?;
        let content =
            serde_json::to_string_pretty(self).map_err(|e| Error::ScheduleError(e.to_string()))?;
        std::fs::write(Self::path(temp_dir), content)?;
        Ok(())
    }

    pub fn append(&mut self, key: &str, updated: Vec<String>, failed: Vec<String>) {
        self.notices.push(ScheduleNotice {
            key: key.to_string(),
            updated,
            failed,
            at: Utc::now(),
            seen: false,
        });
    }

    /// Format the oldest unseen notice (if any) and mark it seen.
    pub fn take_message(&mut self) -> Option<String> {
        let notice = self.notices.iter_mut().find(|n| !n.seen)?;
        notice.seen = true;
        Some(format_notice(notice))
    }
}

fn format_notice(n: &ScheduleNotice) -> String {
    let time = n.at.format("%H:%M UTC");
    if !n.failed.is_empty() && n.updated.is_empty() {
        return format!(
            "machine_setup: schedule {} failed for {} (at {time}). See schedule.log.",
            n.key,
            n.failed.join(", ")
        );
    }
    if !n.failed.is_empty() {
        return format!(
            "machine_setup: updated {} ({}); failed: {} (at {time}). New shells see new binaries; version-manager shells may need a restart.",
            n.updated.join(", "),
            n.key,
            n.failed.join(", ")
        );
    }
    format!(
        "machine_setup: updated {} ({} / {time}). New shells see new binaries; version-manager shells may need a restart.",
        n.updated.join(", "),
        n.key
    )
}

/// Load notices, print one unseen message if present, persist seen flag.
pub fn notify(temp_dir: &Path) -> Result<Option<String>> {
    let mut store = NoticeStore::load(temp_dir)?;
    let msg = store.take_message();
    if msg.is_some() {
        store.save(temp_dir)?;
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn notice_roundtrip_and_seen() {
        let dir = tempdir().unwrap();
        let mut store = NoticeStore::default();
        store.append("0730", vec!["bun".into()], vec![]);
        store.save(dir.path()).unwrap();

        let mut loaded = NoticeStore::load(dir.path()).unwrap();
        let msg = loaded.take_message().unwrap();
        assert!(msg.contains("bun"));
        loaded.save(dir.path()).unwrap();

        let mut again = NoticeStore::load(dir.path()).unwrap();
        assert!(again.take_message().is_none());
    }
}
