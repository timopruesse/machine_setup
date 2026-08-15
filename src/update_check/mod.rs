//! Post-command self update-check notice (GitHub releases + install heuristics).
//!
//! Distinct from Task `update` mode and from schedule `auto_update`.

mod cache;
mod detect;
mod fetch;
mod version;

use std::path::{Path, PathBuf};
use std::process::Stdio;

use chrono::Utc;

use crate::cli::{Command, ScheduleAction};

use cache::UpdateCheckCache;
use detect::detect_from_exe;
use version::is_newer;

const ENV_DISABLE: &str = "MACHINE_SETUP_NO_UPDATE_CHECK";

/// Set by the parent CLI to run a detached cache refresh (no clap / no notice).
pub const ENV_INTERNAL_REFRESH: &str = "MACHINE_SETUP_INTERNAL_UPDATE_REFRESH";

/// Temp dir for [`ENV_INTERNAL_REFRESH`] worker (absolute path string).
pub const ENV_REFRESH_TEMP_DIR: &str = "MACHINE_SETUP_UPDATE_CHECK_TEMP_DIR";

/// Whether the verb should skip the update check entirely.
pub fn should_skip_command(command: &Command) -> bool {
    matches!(
        command,
        Command::Completions { .. }
            | Command::Schema
            | Command::Schedule {
                action: ScheduleAction::Notify { .. },
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

/// Entry for the detached refresh worker (no UI).
pub fn run_internal_refresh_worker() {
    let dir = std::env::var_os(ENV_REFRESH_TEMP_DIR)
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::utils::path::expand_path("~/.machine_setup", None));
    refresh_cache_blocking(&dir);
}

/// Fetch latest tag and persist cache under `temp_dir`. Failures are swallowed.
pub fn refresh_cache_blocking(temp_dir: &Path) {
    refresh_cache_with(temp_dir, fetch_latest);
}

fn refresh_cache_with<F>(temp_dir: &Path, fetch: F)
where
    F: FnOnce() -> crate::error::Result<String>,
{
    let now = Utc::now();
    let mut cache = UpdateCheckCache::load(temp_dir).unwrap_or_default();
    match fetch() {
        Ok(tag) => {
            let stripped = tag.trim().trim_start_matches(['v', 'V']).to_string();
            cache.checked_at = Some(now);
            cache.latest_version = Some(stripped);
            let _ = cache.save(temp_dir);
        }
        Err(_) => {
            cache.checked_at = Some(now);
            let _ = cache.save(temp_dir);
        }
    }
}

/// Run the cached update check and print to stderr if a newer release exists.
///
/// Stale caches schedule a **detached** refresh so the parent CLI does not wait
/// on the network; any previously known `latest_version` can still notify now.
/// Never returns an error to the caller — all failures are swallowed.
pub fn maybe_print_update_notice(command: &Command, temp_dir: &Path, check_for_updates: bool) {
    maybe_print_update_notice_with(
        command,
        temp_dir,
        check_for_updates,
        env!("CARGO_PKG_VERSION"),
        || spawn_background_refresh(temp_dir),
        current_exe_path,
    );
}

fn fetch_latest() -> crate::error::Result<String> {
    fetch::fetch_latest_tag()
}

fn current_exe_path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok()
}

fn spawn_background_refresh(temp_dir: &Path) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.env(ENV_INTERNAL_REFRESH, "1");
    cmd.env(ENV_REFRESH_TEMP_DIR, temp_dir.as_os_str());
    // Don't recurse if the worker somehow re-enters notice paths.
    cmd.env(ENV_DISABLE, "1");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = cmd.spawn();
}

/// Testable entry: inject current version, stale-refresh hook, and exe path.
///
/// `on_stale` is invoked when the cache TTL has expired (production spawns a
/// background worker; tests assert the hook ran).
pub fn maybe_print_update_notice_with<S, E>(
    command: &Command,
    temp_dir: &Path,
    check_for_updates: bool,
    current_version: &str,
    on_stale: S,
    exe: E,
) where
    S: FnOnce(),
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
        // Claim the TTL slot so concurrent invocations do not stampede.
        cache.checked_at = Some(now);
        let _ = cache.save(temp_dir);
        on_stale();
        cache.latest_version.clone()
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
    use cache::TTL_HOURS;
    use chrono::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::tempdir;

    #[test]
    fn skips_schema_and_notify() {
        assert!(should_skip_command(&Command::Schema));
        assert!(should_skip_command(&Command::Schedule {
            action: ScheduleAction::Notify { temp_dir: None }
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
            },
            || None,
        );
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn fresh_cache_skips_stale_hook_but_can_still_notice() {
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
            },
            || Some(std::path::PathBuf::from("/opt/homebrew/bin/machine_setup")),
        );
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn stale_cache_schedules_refresh_without_blocking_on_fetch() {
        let dir = tempdir().unwrap();
        let cache = UpdateCheckCache {
            checked_at: Some(Utc::now() - Duration::hours(TTL_HOURS + 1)),
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
            },
            || None,
        );
        assert!(called.load(Ordering::SeqCst));

        let reloaded = UpdateCheckCache::load(dir.path()).unwrap();
        assert!(reloaded.is_fresh(Utc::now()));
    }

    #[test]
    fn refresh_cache_with_persists_tag() {
        let dir = tempdir().unwrap();
        refresh_cache_with(dir.path(), || Ok("v9.1.0".into()));
        let loaded = UpdateCheckCache::load(dir.path()).unwrap();
        assert_eq!(loaded.latest_version.as_deref(), Some("9.1.0"));
        assert!(loaded.is_fresh(Utc::now()));
    }
}
