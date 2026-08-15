//! Heuristic install-method detection → update command.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    Cargo,
    Scoop,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateHint {
    pub method: InstallMethod,
    /// Primary update one-liner.
    pub command: String,
    /// Optional second line (ambiguous / fallback hint).
    pub hint: Option<String>,
}

pub fn detect_from_exe(exe: &Path) -> UpdateHint {
    let path = exe.to_string_lossy();
    let lower = path.to_ascii_lowercase();

    if looks_like_homebrew(&lower, exe) {
        return UpdateHint {
            method: InstallMethod::Homebrew,
            command: "brew upgrade timopruesse/repo/machine_setup".into(),
            hint: None,
        };
    }

    if looks_like_cargo(&lower, exe) {
        return UpdateHint {
            method: InstallMethod::Cargo,
            command: "cargo install machine_setup --force".into(),
            hint: None,
        };
    }

    if looks_like_scoop(&lower) {
        return UpdateHint {
            method: InstallMethod::Scoop,
            command: "scoop update machine_setup".into(),
            hint: None,
        };
    }

    UpdateHint {
        method: InstallMethod::Binary,
        command: default_reinstall_command(),
        hint: Some(
            "or: brew upgrade timopruesse/repo/machine_setup  /  cargo install machine_setup --force"
                .into(),
        ),
    }
}

fn looks_like_homebrew(lower: &str, exe: &Path) -> bool {
    if lower.contains("homebrew")
        || lower.contains("/cellar/")
        || lower.contains("\\cellar\\")
        || lower.contains("linuxbrew")
    {
        return true;
    }
    // Binary under `brew --prefix` (best-effort; ignore failures)
    if let Ok(out) = std::process::Command::new("brew").arg("--prefix").output() {
        if out.status.success() {
            let prefix = String::from_utf8_lossy(&out.stdout);
            let prefix = prefix.trim();
            if !prefix.is_empty() {
                if let Ok(exe_canon) = exe.canonicalize() {
                    if exe_canon.starts_with(prefix) {
                        return true;
                    }
                }
                if lower.starts_with(&prefix.to_ascii_lowercase()) {
                    return true;
                }
            }
        }
    }
    false
}

fn looks_like_cargo(lower: &str, exe: &Path) -> bool {
    if lower.contains("/.cargo/bin/") || lower.contains("\\.cargo\\bin\\") {
        return true;
    }
    if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        let bin = Path::new(&cargo_home).join("bin");
        if exe.starts_with(&bin) {
            return true;
        }
        if let (Ok(e), Ok(b)) = (exe.canonicalize(), bin.canonicalize()) {
            if e.starts_with(b) {
                return true;
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        let bin = home.join(".cargo").join("bin");
        if exe.starts_with(&bin) {
            return true;
        }
    }
    false
}

fn looks_like_scoop(lower: &str) -> bool {
    lower.contains("\\scoop\\apps\\") || lower.contains("/scoop/apps/")
}

fn default_reinstall_command() -> String {
    #[cfg(windows)]
    {
        "irm https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.ps1 | iex"
            .into()
    }
    #[cfg(not(windows))]
    {
        "curl -fsSL https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.sh | sh"
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn homebrew_cellar() {
        let p = PathBuf::from("/opt/homebrew/Cellar/machine_setup/2.6.1/bin/machine_setup");
        let h = detect_from_exe(&p);
        assert_eq!(h.method, InstallMethod::Homebrew);
        assert!(h.command.contains("brew upgrade"));
        assert!(h.hint.is_none());
    }

    #[test]
    fn cargo_bin() {
        let p = PathBuf::from("/Users/me/.cargo/bin/machine_setup");
        let h = detect_from_exe(&p);
        assert_eq!(h.method, InstallMethod::Cargo);
        assert!(h.command.contains("cargo install"));
    }

    #[test]
    fn unknown_gets_fallback_and_hint() {
        let p = PathBuf::from("/usr/local/bin/machine_setup");
        let h = detect_from_exe(&p);
        assert_eq!(h.method, InstallMethod::Binary);
        assert!(h.hint.is_some());
    }
}
