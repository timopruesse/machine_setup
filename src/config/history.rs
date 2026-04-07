use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskHistory {
    pub installed_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub uninstalled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(flatten)]
    pub tasks: HashMap<String, TaskHistory>,
}

impl History {
    /// Load history from the given directory. Returns empty history if file doesn't exist.
    pub fn load(history_dir: &Path) -> Result<Self> {
        let path = Self::file_path(history_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|e| Error::HistoryError(e.to_string()))
    }

    /// Save history to the given directory.
    pub fn save(&self, history_dir: &Path) -> Result<()> {
        let path = Self::file_path(history_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            serde_json::to_string_pretty(&self).map_err(|e| Error::HistoryError(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Mark a task as installed.
    pub fn mark_installed(&mut self, task_name: &str) {
        let entry = self.tasks.entry(task_name.to_string()).or_default();
        entry.installed_at = Some(Utc::now());
        entry.uninstalled_at = None;
    }

    /// Mark a task as updated.
    pub fn mark_updated(&mut self, task_name: &str) {
        let entry = self.tasks.entry(task_name.to_string()).or_default();
        entry.updated_at = Some(Utc::now());
    }

    /// Mark a task as uninstalled.
    pub fn mark_uninstalled(&mut self, task_name: &str) {
        let entry = self.tasks.entry(task_name.to_string()).or_default();
        entry.uninstalled_at = Some(Utc::now());
        entry.installed_at = None;
    }

    /// Check if a task is currently installed (installed and not uninstalled).
    pub fn is_installed(&self, task_name: &str) -> bool {
        self.tasks
            .get(task_name)
            .is_some_and(|h| h.installed_at.is_some())
    }

    fn file_path(dir: &Path) -> PathBuf {
        dir.join("history.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_history_roundtrip() {
        let dir = tempdir().unwrap();
        let mut history = History::default();

        history.mark_installed("task_a");
        history.mark_installed("task_b");
        history.mark_updated("task_a");
        history.mark_uninstalled("task_b");

        history.save(dir.path()).unwrap();

        let loaded = History::load(dir.path()).unwrap();
        assert!(loaded.is_installed("task_a"));
        assert!(!loaded.is_installed("task_b"));
        assert!(loaded.tasks["task_a"].updated_at.is_some());
        assert!(loaded.tasks["task_b"].uninstalled_at.is_some());
    }

    #[test]
    fn test_history_load_missing_file() {
        let dir = tempdir().unwrap();
        let history = History::load(dir.path()).unwrap();
        assert!(history.tasks.is_empty());
    }

    #[test]
    fn test_install_clears_uninstalled() {
        let mut history = History::default();
        history.mark_uninstalled("task_a");
        assert!(!history.is_installed("task_a"));

        history.mark_installed("task_a");
        assert!(history.is_installed("task_a"));
        assert!(history.tasks["task_a"].uninstalled_at.is_none());
    }
}
