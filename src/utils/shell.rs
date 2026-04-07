use crate::config::types::Shell;
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

/// Build a shell command string with optional profile sourcing and env vars.
pub fn build_shell_command(
    commands: &[String],
    shell: &Shell,
    env: &std::collections::HashMap<String, String>,
) -> String {
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
        let val = if value.starts_with('~') {
            crate::utils::path::expand_path(value, None)
                .to_string_lossy()
                .to_string()
        } else {
            value.clone()
        };
        match shell {
            Shell::Bash | Shell::Zsh => {
                script.push_str(&format!("export {key}=\"{val}\"\n"));
            }
            Shell::PowerShell => {
                script.push_str(&format!("$env:{key} = \"{val}\"\n"));
            }
        }
    }

    for cmd in commands {
        script.push_str(cmd);
        script.push('\n');
    }

    script
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
