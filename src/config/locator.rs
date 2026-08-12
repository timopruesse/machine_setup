//! Config locator — choose a Config document when `-c` is omitted.
//!
//! Search order: working directory, then git repository root. Explicit paths
//! and URLs bypass this module. `init` always writes to the working directory
//! (see `document`); git root is find-only.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use crate::error::{Error, Result};

const BASENAME: &str = "machine_setup";
const EXTENSIONS: &[&str] = &["yaml", "yml", "json"];

/// Resolve an existing Config document: cwd first, then git root.
///
/// When both locations have a file, cwd wins. If git root also has one, a note
/// is written to stderr.
pub fn find(cwd: &Path) -> Result<PathBuf> {
    if let Some(path) = probe_dir(cwd) {
        if let Some(git_root) = find_git_root(cwd) {
            if git_root != cwd {
                if let Some(other) = probe_dir(&git_root) {
                    if other != path {
                        eprintln!(
                            "using {}; also found at {}",
                            path.display(),
                            other.display()
                        );
                    }
                }
            }
        }
        return Ok(path);
    }

    if let Some(git_root) = find_git_root(cwd) {
        if git_root != cwd {
            if let Some(path) = probe_dir(&git_root) {
                return Ok(path);
            }
        }
    }

    Err(Error::ConfigNotLocated)
}

/// Default path for `init` when `-c` is omitted: `./machine_setup.yaml` under cwd.
pub fn init_path(cwd: &Path) -> PathBuf {
    cwd.join(format!("{BASENAME}.yaml"))
}

/// Probe a directory for `machine_setup.{yaml,yml,json}` (yaml preferred).
fn probe_dir(dir: &Path) -> Option<PathBuf> {
    let base = dir.join(BASENAME);
    if base.is_file() {
        return Some(base);
    }
    for ext in EXTENSIONS {
        let candidate = base.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Walk up from `start` looking for a `.git` entry; return that directory.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    // Prefer `git rev-parse` when available (handles worktrees).
    if let Ok(output) = StdCommand::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn find_prefers_cwd_over_git_root() {
        let root = tempdir().unwrap();
        let root_path = root.path();
        fs::create_dir(root_path.join(".git")).unwrap();
        fs::write(root_path.join("machine_setup.yaml"), "tasks: {}\n").unwrap();

        let sub = root_path.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("machine_setup.yaml"), "tasks: {}\n").unwrap();

        let found = find(&sub).unwrap();
        assert_eq!(found, sub.join("machine_setup.yaml"));
    }

    #[test]
    fn find_falls_back_to_git_root() {
        let root = tempdir().unwrap();
        let root_path = root.path();
        fs::create_dir(root_path.join(".git")).unwrap();
        fs::write(root_path.join("machine_setup.yaml"), "tasks: {}\n").unwrap();

        let sub = root_path.join("nested");
        fs::create_dir(&sub).unwrap();

        let found = find(&sub).unwrap();
        assert_eq!(found, root_path.join("machine_setup.yaml"));
    }

    #[test]
    fn find_errors_when_missing() {
        let dir = tempdir().unwrap();
        let err = find(dir.path()).unwrap_err();
        assert!(matches!(err, Error::ConfigNotLocated));
    }

    #[test]
    fn init_path_is_cwd_yaml() {
        let dir = tempdir().unwrap();
        assert_eq!(init_path(dir.path()), dir.path().join("machine_setup.yaml"));
    }

    #[test]
    fn probe_prefers_yaml_over_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("machine_setup.json"), "{}\n").unwrap();
        fs::write(dir.path().join("machine_setup.yaml"), "tasks: {}\n").unwrap();
        assert_eq!(
            probe_dir(dir.path()).unwrap(),
            dir.path().join("machine_setup.yaml")
        );
    }
}
