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

/// One buffered privileged filesystem operation (SudoFs script batch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SudoOp {
    Mkdir(PathBuf),
    Copy { src: PathBuf, dest: PathBuf },
    Symlink { src: PathBuf, dest: PathBuf },
    Remove(PathBuf),
    RemoveDir(PathBuf),
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
        .map_err(|e| Error::Other(format!("Failed to run sudo bash: {e}")))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Other("Failed to open sudo bash stdin".into()))?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| Error::Other(format!("Failed to write sudo script: {e}")))?;
    }

    let status = child
        .wait()
        .map_err(|e| Error::Other(format!("Failed to wait for sudo bash: {e}")))?;

    if !status.success() {
        return Err(Error::Other(format!(
            "sudo bash script failed with exit code {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

fn run_sudo(args: &[&str]) -> Result<()> {
    let status = Command::new("sudo")
        .args(args)
        .status()
        .map_err(|e| Error::Other(format!("Failed to run sudo: {e}")))?;

    if !status.success() {
        return Err(Error::Other(format!(
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
        ]);
        assert!(script.contains("mkdir -p '/t'\n"));
        assert!(script.contains("cp -f '/s/f' '/t/f'\n"));
        assert!(script.contains("ln -sf '/s/l' '/t/l'\n"));
        assert!(script.contains("rm -f '/t/old'\n"));
        assert!(script.contains("rm -rf '/t/dir'\n"));
        assert!(script.starts_with("set -euo pipefail\n"));
    }
}
