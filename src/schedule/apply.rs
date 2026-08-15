//! `schedule apply` / `schedule remove`.

use std::path::{Path, PathBuf};

use crate::config::types::AppConfig;
use crate::error::Result;
use crate::schedule::group;
use crate::schedule::hook;
use crate::schedule::manifest::{ManagedUnit, Manifest};
use crate::schedule::platform::{self, UnitSpec};

#[derive(Debug, Default)]
pub struct ApplyReport {
    pub keys: Vec<String>,
    pub labels: Vec<String>,
    pub hook_script: Option<PathBuf>,
    pub stubs_updated: Vec<PathBuf>,
}

pub fn apply(
    config: &AppConfig,
    config_path: &Path,
    temp_dir: &Path,
    binary: &Path,
    install_hook_stubs: bool,
) -> Result<ApplyReport> {
    let grouped = group::group_keys(config)?;
    let platform = platform::current();
    let mut report = ApplyReport::default();
    let mut new_manifest = Manifest::default();

    let old = Manifest::load(temp_dir)?;
    let desired_labels: std::collections::HashSet<String> =
        grouped.keys().map(|k| platform.label_for_key(k)).collect();

    for (key, tasks) in &grouped {
        let label = platform.label_for_key(key);
        let spec = UnitSpec {
            key: key.clone(),
            binary: binary.to_path_buf(),
            config_path: config_path.to_path_buf(),
        };
        platform.apply_unit(&spec)?;
        report.keys.push(key.as_str().to_string());
        report.labels.push(label.clone());
        new_manifest.units.push(ManagedUnit {
            key: key.as_str().to_string(),
            label,
        });
        let _ = tasks; // selected at run time from Config
    }

    // Remove orphan managed units
    for unit in &old.units {
        if !desired_labels.contains(&unit.label) {
            let _ = platform.remove_unit(&unit.label);
        }
    }

    new_manifest.save(temp_dir)?;

    let hook_path = hook::write_hook_script(temp_dir, binary)?;
    report.hook_script = Some(hook_path.clone());

    if install_hook_stubs {
        if let Some(home) = dirs::home_dir() {
            for rc in hook::default_rc_paths(&home) {
                // Only touch existing rc files (do not create empty .bashrc/.zshrc)
                if rc.exists() {
                    hook::install_stub(&rc, &hook_path)?;
                    report.stubs_updated.push(rc);
                }
            }
        }
    }

    Ok(report)
}

pub fn remove(temp_dir: &Path, keep_hook: bool) -> Result<RemoveReport> {
    let platform = platform::current();
    let manifest = Manifest::load(temp_dir)?;
    let mut removed = Vec::new();
    for unit in &manifest.units {
        platform.remove_unit(&unit.label)?;
        removed.push(unit.label.clone());
    }
    let empty = Manifest::default();
    empty.save(temp_dir)?;

    let mut stubs_cleared = Vec::new();
    if !keep_hook {
        if let Some(home) = dirs::home_dir() {
            for rc in hook::default_rc_paths(&home) {
                if hook::remove_stub(&rc)? {
                    stubs_cleared.push(rc);
                }
            }
        }
        let hook_path = hook::hook_script_path(temp_dir);
        if hook_path.exists() {
            std::fs::remove_file(&hook_path)?;
        }
    }

    Ok(RemoveReport {
        removed_labels: removed,
        stubs_cleared,
    })
}

#[derive(Debug, Default)]
pub struct RemoveReport {
    pub removed_labels: Vec<String>,
    pub stubs_cleared: Vec<PathBuf>,
}
