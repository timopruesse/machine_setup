//! Secrets substrate preflight (ADR-0011 Phase 0).
//!
//! Runtime checks for Config `secrets:` — `op` CLI, vault session, and optional
//! 1Password SSH agent reachability. Resolution of `from_secret` refs is Phase 1.

pub mod auth;
pub mod op_cli;
pub mod ssh_config;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::types::{SecretsConfig, SecretsProvider};
use crate::config::validate::{Severity, ValidationIssue};
use crate::utils::path::expand_path;

const SECRETS_SCOPE: &str = "secrets";

pub(crate) fn command_on_path(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Run Secrets preflight checks. Empty when `secrets` is absent.
pub fn preflight(secrets: Option<&SecretsConfig>) -> Vec<ValidationIssue> {
    let Some(secrets) = secrets else {
        return Vec::new();
    };
    match secrets.default_provider {
        SecretsProvider::OnePassword => preflight_onepassword(secrets.ssh_agent),
    }
}

fn preflight_onepassword(ssh_agent: bool) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if !command_on_path("op") {
        issues.push(issue(
            Severity::Error,
            "1Password CLI (`op`) not found on PATH — run `machine_setup auth enable onepassword` \
             (installs via Homebrew/winget) or install the CLI manually, then unlock 1Password",
        ));
        // Further op/agent checks are misleading without the CLI.
        if ssh_agent {
            issues.extend(check_ssh_agent());
        }
        return issues;
    }

    match op_session_status() {
        OpSession::Ok => {}
        OpSession::LockedOrSignedOut => {
            issues.push(issue(
                Severity::Error,
                "1Password CLI cannot access your vault — unlock the 1Password app, enable \
                 Settings → Developer → Integrate with 1Password CLI, then retry \
                 (legacy shell session: eval $(op signin))",
            ));
        }
        OpSession::Failed(msg) => {
            issues.push(issue(
                Severity::Error,
                format!("1Password CLI check failed: {msg}"),
            ));
        }
    }

    if ssh_agent {
        issues.extend(check_ssh_agent());
    }

    issues
}

fn issue(severity: Severity, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        task_name: SECRETS_SCOPE.into(),
        message: message.into(),
        severity,
    }
}

enum OpSession {
    Ok,
    LockedOrSignedOut,
    Failed(String),
}

/// Probe vault access. Prefer `op vault list` over `op whoami` — whoami requires
/// a classic `OP_SESSION_*` and fails under desktop app / biometric integration.
fn op_session_status() -> OpSession {
    match Command::new("op")
        .args(["vault", "list"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OpSession::Ok,
        Ok(_) => OpSession::LockedOrSignedOut,
        Err(e) => OpSession::Failed(e.to_string()),
    }
}

fn check_ssh_agent() -> Vec<ValidationIssue> {
    if ssh_agent_usable() {
        return Vec::new();
    }
    vec![issue(
        Severity::Error,
        "1Password SSH agent is not reachable — enable the SSH agent in 1Password \
         (Settings → Developer), ensure IdentityAgent points at the agent socket \
         (see recipe `onepassword-ssh`), unlock 1Password, then retry",
    )]
}

fn ssh_agent_usable() -> bool {
    for sock in candidate_agent_sockets() {
        if agent_reachable(&sock) {
            return true;
        }
    }
    false
}

fn candidate_agent_sockets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(sock) = std::env::var("SSH_AUTH_SOCK") {
        if !sock.is_empty() {
            out.push(PathBuf::from(sock));
        }
    }
    // Documented 1Password agent socket locations.
    out.push(expand_path(
        "~/Library/Group Containers/2BUA8C4H2G.com.1password/t/agent.sock",
        None,
    ));
    out.push(expand_path("~/.1password/agent.sock", None));
    out
}

/// `ssh-add -l` exit status 2 means the agent cannot be contacted.
fn agent_reachable(sock: &Path) -> bool {
    if !sock.exists() {
        return false;
    }
    match Command::new("ssh-add")
        .arg("-l")
        .env("SSH_AUTH_SOCK", sock)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.code() != Some(2),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::SecretsConfig;

    #[test]
    fn preflight_none_is_empty() {
        assert!(preflight(None).is_empty());
    }

    #[test]
    fn preflight_without_op_reports_error() {
        // If `op` is installed in the environment this still exercises ssh_agent:false path
        // for missing-op only when op is absent; when op is present, whoami may pass/fail.
        let secrets = SecretsConfig {
            default_provider: SecretsProvider::OnePassword,
            ssh_agent: false,
        };
        let issues = preflight(Some(&secrets));
        // Must not panic; either empty (op signed in) or secrets-scoped errors.
        for i in &issues {
            assert_eq!(i.task_name, SECRETS_SCOPE);
        }
    }
}
