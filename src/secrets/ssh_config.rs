//! Immediate `~/.ssh/config` IdentityAgent wiring for the 1Password SSH agent.
//!
//! Used by `machine_setup auth enable onepassword --ssh-agent` (and documented
//! for the `onepassword-ssh` recipe's install-time script).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::utils::path::expand_path;

/// Default macOS 1Password SSH agent socket (tilde form for ssh config).
pub const MACOS_AGENT_TILDE: &str =
    "~/Library/Group Containers/2BUA8C4H2G.com.1password/t/agent.sock";

/// Default Linux 1Password SSH agent socket (tilde form for ssh config).
pub const LINUX_AGENT_TILDE: &str = "~/.1password/agent.sock";

const MARKER: &str = "machine_setup onepassword-ssh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireOutcome {
    /// IdentityAgent line already present.
    AlreadyConfigured,
    /// Appended Host * / IdentityAgent block.
    Added,
}

/// Ensure `~/.ssh/config` points IdentityAgent at the platform 1Password socket.
pub fn ensure_identity_agent(home: Option<&Path>) -> Result<WireOutcome> {
    let ssh_dir = home_ssh_dir(home)?;
    fs::create_dir_all(&ssh_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&ssh_dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&ssh_dir, perms)?;
    }

    let config_path = ssh_dir.join("config");
    if !config_path.exists() {
        fs::File::create(&config_path)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&config_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&config_path, perms)?;
    }

    let existing = fs::read_to_string(&config_path)?;
    if identity_agent_configured(&existing) {
        return Ok(WireOutcome::AlreadyConfigured);
    }

    let agent = agent_tilde_for_host();
    let block = format!("\n# {MARKER}\nHost *\n\tIdentityAgent \"{agent}\"\n");
    let mut file = OpenOptions::new().append(true).open(&config_path)?;
    file.write_all(block.as_bytes())?;
    Ok(WireOutcome::Added)
}

fn home_ssh_dir(home: Option<&Path>) -> Result<PathBuf> {
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => expand_path("~", None),
    };
    if home.as_os_str().is_empty() {
        return Err(Error::PathError("cannot resolve home directory".into()));
    }
    Ok(home.join(".ssh"))
}

fn agent_tilde_for_host() -> &'static str {
    if cfg!(target_os = "macos") {
        MACOS_AGENT_TILDE
    } else {
        LINUX_AGENT_TILDE
    }
}

fn identity_agent_configured(contents: &str) -> bool {
    contents.lines().any(|line| {
        let t = line.trim();
        t.starts_with("IdentityAgent") && (t.contains("1password") || t.contains("2BUA8C4H2G"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_adds_then_idempotent() {
        let dir = tempdir().unwrap();
        let first = ensure_identity_agent(Some(dir.path())).unwrap();
        assert_eq!(first, WireOutcome::Added);
        let config = fs::read_to_string(dir.path().join(".ssh/config")).unwrap();
        assert!(config.contains("IdentityAgent"));
        assert!(config.contains("1password") || config.contains("2BUA8C4H2G"));

        let second = ensure_identity_agent(Some(dir.path())).unwrap();
        assert_eq!(second, WireOutcome::AlreadyConfigured);
        let again = fs::read_to_string(dir.path().join(".ssh/config")).unwrap();
        assert_eq!(
            again.matches("IdentityAgent").count(),
            config.matches("IdentityAgent").count()
        );
    }

    #[test]
    fn detects_existing_identity_agent() {
        assert!(identity_agent_configured(
            "Host *\n\tIdentityAgent \"~/.1password/agent.sock\"\n"
        ));
        assert!(!identity_agent_configured(
            "Host *\n\tIdentityFile ~/.ssh/id_ed25519\n"
        ));
    }
}
