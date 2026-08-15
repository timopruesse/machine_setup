//! Managed OS unit ids for idempotent schedule apply/remove.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedUnit {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub units: Vec<ManagedUnit>,
}

impl Manifest {
    fn path(temp_dir: &Path) -> PathBuf {
        temp_dir.join("schedule_manifest.json")
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn manifest_roundtrip() {
        let dir = tempdir().unwrap();
        let m = Manifest {
            units: vec![ManagedUnit {
                key: "0730".into(),
                label: "com.machine_setup.schedule.0730".into(),
            }],
        };
        m.save(dir.path()).unwrap();
        let loaded = Manifest::load(dir.path()).unwrap();
        assert_eq!(loaded.units, m.units);
    }
}
