//! Persist last self update-check attempt.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::Result;

pub const TTL_HOURS: i64 = 24;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    pub checked_at: Option<DateTime<Utc>>,
    pub latest_version: Option<String>,
}

impl UpdateCheckCache {
    fn path(temp_dir: &Path) -> PathBuf {
        temp_dir.join("update_check.json")
    }

    pub fn load(temp_dir: &Path) -> Result<Self> {
        let path = Self::path(temp_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, temp_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(temp_dir)?;
        let content = serde_json::to_string(self)?;
        std::fs::write(Self::path(temp_dir), content)?;
        Ok(())
    }

    /// True if `checked_at` is within TTL.
    pub fn is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.checked_at
            .is_some_and(|t| now - t < Duration::hours(TTL_HOURS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_and_ttl() {
        let dir = tempdir().unwrap();
        let mut cache = UpdateCheckCache {
            checked_at: Some(Utc::now()),
            latest_version: Some("2.7.0".into()),
        };
        cache.save(dir.path()).unwrap();
        let loaded = UpdateCheckCache::load(dir.path()).unwrap();
        assert_eq!(loaded.latest_version.as_deref(), Some("2.7.0"));
        assert!(loaded.is_fresh(Utc::now()));

        cache.checked_at = Some(Utc::now() - Duration::hours(TTL_HOURS + 1));
        assert!(!cache.is_fresh(Utc::now()));
    }
}
