//! Post-command self update-check notice (GitHub releases + install heuristics).
//!
//! Distinct from Task `update` mode and from schedule `auto_update`.

mod cache;
mod detect;
mod fetch;
mod version;

use std::path::Path;

use chrono::Utc;

use crate::cli::{Command, ScheduleAction};

use cache::UpdateCheckCache;
use detect::detect_from_exe;
use version::is_newer;

const ENV_DISABLE: &str = "MACHINE_SETUP_NO_UPDATE_CHECK";

/// Whether the verb should skip the update check entirely.
pub fn should_skip_command(command: &Command) -> bool {
    matches!(
        command,
        Command::Completions { .. }
            | Command::Schema
            | Command::Schedule {
                action: ScheduleAction::Notify,
            }
    )
}

fn env_disables() -> bool {
    match std::env::var(ENV_DISABLE) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes")
        }
        Err(_) => false,
    }
}

/// Run the cached update check and print to stderr if a newer release exists.
/// Never returns an error to the caller — all failures are swallowed.
pub fn maybe_print_update_notice(command: &Command, temp_dir: &Path, check_for_updates: bool) {
    maybe_print_update_notice_with(
        command,
        temp_dir,
        check_for_updates,
        env!("CARGO_PKG_VERSION"),
        fetch_latest,
        current_exe_path,
    );
}

fn fetch_latest() -> crate::error::Result<String> {
    fetch::fetch_latest_tag()
}

fn current_exe_path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok()
}

/// Testable entry: inject current version, fetch, and exe path.
pub fn maybe_print_update_notice_with<F, E>(
    command: &Command,
    temp_dir: &Path,
    check_for_updates: bool,
    current_version: &str,
    fetch: F,
    exe: E,
) where
    F: FnOnce() -> crate::error::Result<String>,
    E: FnOnce() -> Option<std::path::PathBuf>,
{
    if should_skip_command(command) || !check_for_updates || env_disables() {
        return;
    }

    let now = Utc::now();
    let mut cache = UpdateCheckCache::load(temp_dir).unwrap_or_default();

    let latest = if cache.is_fresh(now) {
        cache.latest_version.clone()
    } else {
        match fetch() {
            Ok(tag) => {
                let stripped = tag.trim().trim_start_matches(['v', 'V']).to_string();
                cache.checked_at = Some(now);
                cache.latest_version = Some(stripped.clone());
                let _ = cache.save(temp_dir);
                Some(stripped)
            }
            Err(_) => {
                cache.checked_at = Some(now);
                let _ = cache.save(temp_dir);
                cache.latest_version.clone()
            }
        }
    };

    let Some(latest) = latest else {
        return;
    };

    if !is_newer(&latest, current_version) {
        return;
    }

    let hint = exe()
        .map(|p| detect_from_exe(&p))
        .unwrap_or_else(|| detect_from_exe(Path::new("machine_setup")));

    eprintln!(
        "machine_setup: new version {latest} available (you have {current_version}).\n  Update: {}",
        hint.command
    );
    if let Some(extra) = hint.hint {
        eprintln!("  {extra}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::tempdir;

    #[test]
    fn skips_schema_and_notify() {
        assert!(should_skip_command(&Command::Schema));
        assert!(should_skip_command(&Command::Schedule {
            action: ScheduleAction::Notify
        }));
        assert!(!should_skip_command(&Command::List));
    }

    #[test]
    fn config_false_skips_fetch() {
        let dir = tempdir().unwrap();
        let called = AtomicBool::new(false);
        maybe_print_update_notice_with(
            &Command::List,
            dir.path(),
            false,
            "2.6.1",
            || {
                called.store(true, Ordering::SeqCst);
                Ok("9.0.0".into())
            },
            || None,
        );
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn fresh_cache_skips_fetch_but_can_still_notice() {
        let dir = tempdir().unwrap();
        let cache = UpdateCheckCache {
            checked_at: Some(Utc::now()),
            latest_version: Some("9.0.0".into()),
        };
        cache.save(dir.path()).unwrap();

        let called = AtomicBool::new(false);
        maybe_print_update_notice_with(
            &Command::List,
            dir.path(),
            true,
            "2.6.1",
            || {
                called.store(true, Ordering::SeqCst);
                Ok("9.0.0".into())
            },
            || Some(std::path::PathBuf::from("/opt/homebrew/bin/machine_setup")),
        );
        assert!(!called.load(Ordering::SeqCst));
    }
}
