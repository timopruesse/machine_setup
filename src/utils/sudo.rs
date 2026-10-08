use crate::error::{Error, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Copy a file using `sudo cp`.
pub fn sudo_copy(src: &Path, dest: &Path) -> Result<()> {
    run_sudo(&["cp", "-f", &src.to_string_lossy(), &dest.to_string_lossy()])
}

/// Copy a directory tree into `target` with one `sudo cp -a` (contents of `src`).
pub fn sudo_copy_tree(src: &Path, target: &Path) -> Result<()> {
    sudo_mkdir(target)?;
    let src_contents = format!("{}/.", src.to_string_lossy());
    run_sudo(&["cp", "-a", &src_contents, &target.to_string_lossy()])
}

/// Create a symlink using `sudo ln -sf`.
pub fn sudo_symlink(src: &Path, dest: &Path) -> Result<()> {
    run_sudo(&["ln", "-sf", &src.to_string_lossy(), &dest.to_string_lossy()])
}

/// Remove a file using `sudo rm -f`.
pub fn sudo_remove(path: &Path) -> Result<()> {
    run_sudo(&["rm", "-f", &path.to_string_lossy()])
}

/// Remove a directory using `sudo rm -rf`.
pub fn sudo_remove_dir(path: &Path) -> Result<()> {
    run_sudo(&["rm", "-rf", &path.to_string_lossy()])
}

/// Create a directory using `sudo mkdir -p`.
pub fn sudo_mkdir(path: &Path) -> Result<()> {
    run_sudo(&["mkdir", "-p", &path.to_string_lossy()])
}

/// Rename/move a path using `sudo mv -f`.
pub fn sudo_rename(src: &Path, dest: &Path) -> Result<()> {
    run_sudo(&["mv", "-f", &src.to_string_lossy(), &dest.to_string_lossy()])
}

/// One buffered privileged filesystem operation (SudoFs script batch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SudoOp {
    Mkdir(PathBuf),
    Copy { src: PathBuf, dest: PathBuf },
    Symlink { src: PathBuf, dest: PathBuf },
    Remove(PathBuf),
    RemoveDir(PathBuf),
    Rename { src: PathBuf, dest: PathBuf },
}

/// Shell-escape a path for single-quoted use in a bash script.
pub fn sh_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Render buffered ops as a bash script (one sudo process via [`sudo_bash_script`]).
pub fn build_sudo_script(ops: &[SudoOp]) -> String {
    let mut script = String::from("set -euo pipefail\n");
    for op in ops {
        match op {
            SudoOp::Mkdir(path) => {
                script.push_str(&format!("mkdir -p {}\n", sh_quote(path)));
            }
            SudoOp::Copy { src, dest } => {
                script.push_str(&format!("cp -f {} {}\n", sh_quote(src), sh_quote(dest)));
            }
            SudoOp::Symlink { src, dest } => {
                script.push_str(&format!("ln -sf {} {}\n", sh_quote(src), sh_quote(dest)));
            }
            SudoOp::Remove(path) => {
                script.push_str(&format!("rm -f {}\n", sh_quote(path)));
            }
            SudoOp::RemoveDir(path) => {
                script.push_str(&format!("rm -rf {}\n", sh_quote(path)));
            }
            SudoOp::Rename { src, dest } => {
                script.push_str(&format!("mv -f {} {}\n", sh_quote(src), sh_quote(dest)));
            }
        }
    }
    script
}

/// Run a bash script under a single `sudo bash -s`.
pub fn sudo_bash_script(script: &str) -> Result<()> {
    let mut child = Command::new("sudo")
        .args(["bash", "-s"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| Error::SudoFailed(format!("Failed to run sudo bash: {e}")))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| Error::SudoFailed("Failed to open sudo bash stdin".into()))?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| Error::SudoFailed(format!("Failed to write sudo script: {e}")))?;
    }

    let status = child
        .wait()
        .map_err(|e| Error::SudoFailed(format!("Failed to wait for sudo bash: {e}")))?;

    if !status.success() {
        return Err(Error::SudoFailed(format!(
            "sudo bash script failed with exit code {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

/// How often [`spawn_keepalive`] refreshes cached sudo credentials. Well under
/// the default 15-minute `timestamp_timeout` (sudo and sudo-rs).
pub const KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Cache sudo credentials (`sudo -v`) so `run` commands that call `sudo` reuse
/// them instead of prompting. Prompts only when stdin is a terminal.
///
/// Returns true when credentials are cached afterwards.
pub fn pre_authenticate() -> bool {
    use std::io::IsTerminal;

    if sudo_cached() {
        return true;
    }
    if !std::io::stdin().is_terminal() {
        return false;
    }

    eprintln!("Some tasks require sudo. Please enter your password:");
    Command::new("sudo")
        .arg("-v")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .is_ok_and(|s| s.success())
}

fn sudo_cached() -> bool {
    Command::new("sudo")
        .args(["-n", "-v"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Refresh cached sudo credentials every [`KEEPALIVE_INTERVAL`] so long runs
/// do not outlive the sudo timestamp. Abort the handle when execution ends.
pub fn spawn_keepalive() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(KEEPALIVE_INTERVAL);
        interval.tick().await;
        loop {
            interval.tick().await;
            let _ = tokio::process::Command::new("sudo")
                .args(["-n", "-v"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
    })
}

fn run_sudo(args: &[&str]) -> Result<()> {
    let status = Command::new("sudo")
        .args(args)
        .status()
        .map_err(|e| Error::SudoFailed(format!("Failed to run sudo: {e}")))?;

    if !status.success() {
        return Err(Error::SudoFailed(format!(
            "sudo {} failed with exit code {}",
            args.join(" "),
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_escapes_single_quotes() {
        assert_eq!(sh_quote(Path::new("a'b")), r"'a'\''b'");
    }

    #[test]
    fn build_sudo_script_orders_ops() {
        let script = build_sudo_script(&[
            SudoOp::Mkdir(PathBuf::from("/t")),
            SudoOp::Copy {
                src: PathBuf::from("/s/f"),
                dest: PathBuf::from("/t/f"),
            },
            SudoOp::Symlink {
                src: PathBuf::from("/s/l"),
                dest: PathBuf::from("/t/l"),
            },
            SudoOp::Remove(PathBuf::from("/t/old")),
            SudoOp::RemoveDir(PathBuf::from("/t/dir")),
            SudoOp::Rename {
                src: PathBuf::from("/t/orig"),
                dest: PathBuf::from("/t/orig.bak"),
            },
        ]);
        assert!(script.contains("mkdir -p '/t'\n"));
        assert!(script.contains("cp -f '/s/f' '/t/f'\n"));
        assert!(script.contains("ln -sf '/s/l' '/t/l'\n"));
        assert!(script.contains("rm -f '/t/old'\n"));
        assert!(script.contains("rm -rf '/t/dir'\n"));
        assert!(script.contains("mv -f '/t/orig' '/t/orig.bak'\n"));
        assert!(script.starts_with("set -euo pipefail\n"));
    }
}
