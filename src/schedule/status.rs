//! Human-readable `schedule status`.

use std::path::Path;

use crate::config::types::AppConfig;
use crate::error::Result;
use crate::schedule::group;
use crate::schedule::manifest::Manifest;
use crate::schedule::notices::NoticeStore;
use crate::schedule::platform;

pub fn render_status(config: &AppConfig, temp_dir: &Path) -> Result<String> {
    let grouped = group::group_keys(config)?;
    let manifest = Manifest::load(temp_dir).unwrap_or_default();
    let platform = platform::current();
    let notices = NoticeStore::load(temp_dir).unwrap_or_default();

    let mut out = String::new();
    if grouped.is_empty() {
        out.push_str("No tasks declare auto_update.\n");
    } else {
        out.push_str("Configured schedules:\n");
        for (key, tasks) in &grouped {
            let label = platform.label_for_key(key);
            let managed = manifest.units.iter().any(|u| u.key == key.as_str());
            out.push_str(&format!(
                "  {} (key {}) — tasks: {} — unit: {} — {}\n",
                key,
                key.as_str(),
                tasks.join(", "),
                label,
                if managed {
                    "in manifest"
                } else {
                    "not applied (run schedule apply)"
                }
            ));
        }
    }

    out.push_str("\nManifest units:\n");
    if manifest.units.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for u in &manifest.units {
            out.push_str(&format!("  {} → {}\n", u.key, u.label));
        }
    }

    let unseen = notices.notices.iter().filter(|n| !n.seen).count();
    out.push_str(&format!("\nUnseen notices: {unseen}\n"));
    Ok(out)
}
