//! `machine_setup auth` — enable/disable/status for the Secrets substrate (ADR-0011).

use std::path::Path;

use crate::config::document_edit::{self, write_config};
use crate::config::load_config;
use crate::config::types::{SecretsConfig, SecretsProvider};
use crate::config::validate::Severity;
use crate::error::Result;

use super::op_cli::{self, OpCliOutcome};
use super::preflight;
use super::ssh_config::{self, WireOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableOptions {
    /// Opt into SSH agent preflight (default true for CLI).
    pub ssh_agent: bool,
    /// Write IdentityAgent into `~/.ssh/config` when `ssh_agent` is true.
    pub wire_ssh_config: bool,
    /// Install `op` via Homebrew/winget when missing.
    pub install_cli: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnableReport {
    pub secrets_changed: bool,
    pub ssh_agent: bool,
    pub wire: Option<WireOutcome>,
    pub op_cli: Option<OpCliOutcome>,
}

/// Set root `secrets:` for 1Password; optionally install `op` and wire SSH.
///
/// Rewrites the Config document via serde (comments/formatting may change),
/// same as `remove` / `replace`.
pub fn enable_onepassword(path: &Path, opts: EnableOptions) -> Result<EnableReport> {
    document_edit::ensure_yaml_for_auth(path)?;

    let op_cli = if opts.install_cli {
        Some(op_cli::ensure_installed()?)
    } else {
        None
    };

    let mut config = load_config(path.to_str().unwrap_or_default())?;

    let desired = SecretsConfig {
        default_provider: SecretsProvider::OnePassword,
        ssh_agent: opts.ssh_agent,
    };
    let secrets_changed = config.secrets.as_ref() != Some(&desired);
    config.secrets = Some(desired);
    write_config(path, &config)?;

    let wire = if opts.ssh_agent && opts.wire_ssh_config {
        Some(ssh_config::ensure_identity_agent(None)?)
    } else {
        None
    };

    Ok(EnableReport {
        secrets_changed,
        ssh_agent: opts.ssh_agent,
        wire,
        op_cli,
    })
}

/// Clear the root `secrets:` block.
pub fn disable(path: &Path) -> Result<bool> {
    document_edit::ensure_yaml_for_auth(path)?;
    let mut config = load_config(path.to_str().unwrap_or_default())?;
    let had = config.secrets.take().is_some();
    if had {
        write_config(path, &config)?;
    }
    Ok(had)
}

/// Human-readable status lines for `auth status` (config + preflight).
pub fn status_lines(path: &Path) -> Result<Vec<String>> {
    let config = load_config(path.to_str().unwrap_or_default())?;
    let mut lines = Vec::new();

    match &config.secrets {
        None => {
            lines.push("secrets: not configured".into());
            lines.push("Hint: machine_setup auth enable onepassword".into());
        }
        Some(s) => {
            lines.push(format!(
                "secrets: provider={} ssh_agent={}",
                s.default_provider, s.ssh_agent
            ));
            let issues = preflight(Some(s));
            if issues.is_empty() {
                lines.push("preflight: ok".into());
            } else {
                for issue in issues {
                    lines.push(format!(
                        "preflight [{}]: {}",
                        match issue.severity {
                            Severity::Error => "ERROR",
                            Severity::Warning => "WARN",
                        },
                        issue.message
                    ));
                }
            }
        }
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::document;
    use tempfile::tempdir;

    fn opts(ssh_agent: bool) -> EnableOptions {
        EnableOptions {
            ssh_agent,
            wire_ssh_config: false,
            install_cli: false,
        }
    }

    #[test]
    fn enable_writes_secrets_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();

        let r1 = enable_onepassword(&path, opts(true)).unwrap();
        assert!(r1.secrets_changed);
        assert!(r1.ssh_agent);
        assert!(r1.wire.is_none());
        assert!(r1.op_cli.is_none());

        let cfg = load_config(path.to_str().unwrap()).unwrap();
        let s = cfg.secrets.unwrap();
        assert_eq!(s.default_provider, SecretsProvider::OnePassword);
        assert!(s.ssh_agent);

        let r2 = enable_onepassword(&path, opts(true)).unwrap();
        assert!(!r2.secrets_changed);
    }

    #[test]
    fn disable_clears_secrets() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        enable_onepassword(&path, opts(false)).unwrap();
        assert!(disable(&path).unwrap());
        let cfg = load_config(path.to_str().unwrap()).unwrap();
        assert!(cfg.secrets.is_none());
        assert!(!disable(&path).unwrap());
    }

    #[test]
    fn status_without_secrets_hints_enable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let lines = status_lines(&path).unwrap();
        assert!(lines.iter().any(|l| l.contains("not configured")));
        assert!(lines.iter().any(|l| l.contains("auth enable onepassword")));
    }
}
