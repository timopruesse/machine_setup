use crate::config::types::Shell;
use crate::error::{Error, Result};
use std::path::Path;

/// Get the shell binary path.
pub fn shell_binary(shell: &Shell) -> &str {
    match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
        Shell::PowerShell => {
            if cfg!(windows) {
                "powershell"
            } else {
                "pwsh"
            }
        }
    }
}

/// Get the shell profile file path.
pub fn shell_profile(shell: &Shell) -> Option<String> {
    let home = dirs::home_dir()?;
    let profile = match shell {
        Shell::Bash => home.join(".bashrc"),
        Shell::Zsh => home.join(".zshrc"),
        Shell::PowerShell => return None, // PowerShell handles profiles differently
    };
    if profile.exists() {
        Some(profile.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Check that an environment variable key is a valid identifier.
pub fn validate_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Escape a value for use inside a bash/zsh single-quoted string.
/// Wraps in single quotes, replacing internal `'` with `'\''`.
fn escape_shell_value(val: &str) -> String {
    let escaped = val.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Escape a value for use inside a PowerShell double-quoted string.
/// Escapes `"` as `` `" `` and `$` as `` `$ ``.
fn escape_powershell_value(val: &str) -> String {
    let escaped = val
        .replace('`', "``")
        .replace('"', "`\"")
        .replace('$', "`$");
    format!("\"{escaped}\"")
}

/// Build a shell command string with optional profile sourcing and env vars.
pub fn build_shell_command(
    commands: &[String],
    shell: &Shell,
    env: &std::collections::HashMap<String, String>,
) -> Result<String> {
    let mut script = String::new();

    // Source profile if available (not for PowerShell)
    if let Some(profile) = shell_profile(shell) {
        match shell {
            Shell::Bash | Shell::Zsh => {
                script.push_str(&format!("source \"{profile}\"\n"));
            }
            Shell::PowerShell => {}
        }
    }

    // Export environment variables into the script
    // Only expand ~ in values (home dir), don't prepend config_dir for relative paths
    for (key, value) in env {
        if !validate_env_key(key) {
            return Err(Error::ShellFailed(format!(
                "Invalid environment variable name: {key:?}"
            )));
        }
        let val = if value.starts_with('~') {
            crate::utils::path::expand_path(value, None)
                .to_string_lossy()
                .into_owned()
        } else {
            value.clone()
        };
        match shell {
            Shell::Bash | Shell::Zsh => {
                script.push_str(&format!("export {key}={}\n", escape_shell_value(&val)));
            }
            Shell::PowerShell => {
                script.push_str(&format!("$env:{key} = {}\n", escape_powershell_value(&val)));
            }
        }
    }

    for cmd in commands {
        script.push_str(cmd);
        script.push('\n');
    }

    Ok(script)
}

/// Get the script file extension.
pub fn script_extension(shell: &Shell) -> &str {
    match shell {
        Shell::Bash | Shell::Zsh => "sh",
        Shell::PowerShell => "ps1",
    }
}

/// Write a temporary script file and return its path.
pub fn write_temp_script(
    content: &str,
    shell: &Shell,
    temp_dir: &Path,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(temp_dir)?;
    let filename = format!(
        "ms_{}_{}.{}",
        std::process::id(),
        rand_suffix(),
        script_extension(shell)
    );
    let path = temp_dir.join(filename);
    std::fs::write(&path, content)?;

    // Set execute permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(path)
}

fn rand_suffix() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_validate_env_key_valid() {
        assert!(validate_env_key("MY_VAR"));
        assert!(validate_env_key("_PRIVATE"));
        assert!(validate_env_key("a"));
        assert!(validate_env_key("PATH"));
        assert!(validate_env_key("var123"));
    }

    #[test]
    fn test_validate_env_key_invalid() {
        assert!(!validate_env_key(""));
        assert!(!validate_env_key("123abc"));
        assert!(!validate_env_key("my-var"));
        assert!(!validate_env_key("my var"));
        assert!(!validate_env_key("foo=bar"));
        assert!(!validate_env_key("$VAR"));
    }

    #[test]
    fn test_escape_shell_value_simple() {
        assert_eq!(escape_shell_value("hello"), "'hello'");
    }

    #[test]
    fn test_escape_shell_value_with_single_quote() {
        assert_eq!(escape_shell_value("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_escape_shell_value_prevents_injection() {
        // These should all be rendered as literal strings, not executed
        assert_eq!(escape_shell_value("$(whoami)"), "'$(whoami)'");
        assert_eq!(escape_shell_value("`rm -rf`"), "'`rm -rf`'");
        assert_eq!(
            escape_shell_value("val\"; echo pwned"),
            "'val\"; echo pwned'"
        );
        assert_eq!(escape_shell_value("$HOME"), "'$HOME'");
        assert_eq!(escape_shell_value("line1\nline2"), "'line1\nline2'");
    }

    #[test]
    fn test_escape_powershell_value_simple() {
        assert_eq!(escape_powershell_value("hello"), "\"hello\"");
    }

    #[test]
    fn test_escape_powershell_value_prevents_injection() {
        assert_eq!(escape_powershell_value("$env:SECRET"), "\"`$env:SECRET\"");
        assert_eq!(
            escape_powershell_value("val\"; Write-Host pwned"),
            "\"val`\"; Write-Host pwned\""
        );
    }

    #[test]
    fn test_build_shell_command_escapes_env() {
        let mut env = HashMap::new();
        env.insert("MY_VAR".to_string(), "$(whoami)".to_string());

        let script =
            build_shell_command(&["echo $MY_VAR".to_string()], &Shell::Bash, &env).unwrap();

        assert!(script.contains("export MY_VAR='$(whoami)'"));
    }

    #[test]
    fn test_build_shell_command_rejects_invalid_key() {
        let mut env = HashMap::new();
        env.insert("INVALID-KEY".to_string(), "value".to_string());

        let result = build_shell_command(&["echo test".to_string()], &Shell::Bash, &env);
        assert!(result.is_err());
    }
}
