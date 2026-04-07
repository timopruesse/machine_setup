use crate::error::{Error, Result};
use std::path::Path;
use std::process::Command;

/// Copy a file using `sudo cp`.
pub fn sudo_copy(src: &Path, dest: &Path) -> Result<()> {
    run_sudo(&["cp", "-f", &src.to_string_lossy(), &dest.to_string_lossy()])
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
