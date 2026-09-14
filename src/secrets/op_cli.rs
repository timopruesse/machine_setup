//! Install / detect the 1Password CLI (`op`).

use std::process::Command;

use crate::error::{Error, Result};

use super::command_on_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpCliOutcome {
    /// `op` already on PATH.
    AlreadyPresent,
    /// Installed via the named package manager.
    Installed { method: &'static str },
}

/// Ensure `op` is available. When missing, install via Homebrew or winget.
pub fn ensure_installed() -> Result<OpCliOutcome> {
    if command_on_path("op") {
        return Ok(OpCliOutcome::AlreadyPresent);
    }

    if command_on_path("brew") {
        eprintln!("Installing 1Password CLI via Homebrew…");
        run_install(
            "brew",
            &["install", "--cask", "1password-cli"],
            "Homebrew (`brew install --cask 1password-cli`)",
        )?;
        return verify_after_install("Homebrew");
    }

    #[cfg(windows)]
    {
        if command_on_path("winget") {
            eprintln!("Installing 1Password CLI via winget…");
            run_install(
                "winget",
                &[
                    "install",
                    "-e",
                    "--id",
                    "AgileBits.1Password.CLI",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                ],
                "winget (`winget install -e --id AgileBits.1Password.CLI`)",
            )?;
            return verify_after_install("winget");
        }
    }

    Err(Error::PathError(
        "1Password CLI (`op`) is not on PATH and no supported installer was found. \
         Install it manually (macOS/Linux: `brew install --cask 1password-cli`; \
         Windows: `winget install -e --id AgileBits.1Password.CLI`), then re-run \
         `machine_setup auth enable onepassword`."
            .into(),
    ))
}

fn run_install(program: &str, args: &[&str], label: &str) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| Error::PathError(format!("failed to run {program}: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::PathError(format!(
            "failed to install 1Password CLI via {label} (exit {status})"
        )))
    }
}

fn verify_after_install(method: &'static str) -> Result<OpCliOutcome> {
    if command_on_path("op") {
        Ok(OpCliOutcome::Installed { method })
    } else {
        Err(Error::PathError(format!(
            "1Password CLI install via {method} finished, but `op` is still not on PATH — \
             open a new shell or check your PATH, then re-run auth enable"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_when_op_present_is_already() {
        // CI / developer machines may or may not have `op`; only assert the
        // AlreadyPresent arm when it is available.
        if command_on_path("op") {
            assert_eq!(ensure_installed().unwrap(), OpCliOutcome::AlreadyPresent);
        }
    }
}
