use core::fmt;
use std::{fs::File, str::FromStr};

use ergo_fs::IoWrite;
use regex::Regex;

use super::temp_storage::create_temp_file;

#[derive(Debug, Clone, Copy)]
pub enum Shell {
    Zsh,
    Bash,
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Shell::Bash => write!(f, "bash"),
            Shell::Zsh => write!(f, "zsh"),
        }
    }
}

impl FromStr for Shell {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "zsh" => Ok(Shell::Zsh),
            "bash" => Ok(Shell::Bash),
            _ => Err(format!("Unknown shell: {s}")),
        }
    }
}

const BASH_STR: &str = "#!/bin/bash\nsource $HOME/.bashrc >/dev/null 2>&1\n";
const ZSH_STR: &str = "#!/bin/zsh\nsource $HOME/.zshrc >/dev/null 2>&1\n";

#[cfg(target_family = "windows")]
fn make_executable(file: &mut File) -> Result<(), String> {
    // TODO: How to set permissions for Windows?
    Ok(())
}

#[cfg(target_family = "unix")]
fn make_executable(file: &mut File) -> Result<(), String> {
    let perm_result = file.set_permissions(
        <std::fs::Permissions as std::os::unix::prelude::PermissionsExt>::from_mode(0o755),
    );
    if let Err(err_perm) = perm_result {
        return Err(err_perm.to_string());
    }

    Ok(())
}

pub fn create_script_file(
    shell: Shell,
    commands: Vec<String>,
    temp_dir: &str,
) -> Result<String, String> {
    let temp_file = create_temp_file("sh", temp_dir)?;
    let mut file = temp_file.file;
    let path = temp_file.path;

    match shell {
        Shell::Bash => file.write_all(String::from(BASH_STR).as_bytes()).unwrap(),
        Shell::Zsh => file.write_all(String::from(ZSH_STR).as_bytes()).unwrap(),
    }

    for command in commands {
        file.write_all(format!("{command}\n").as_bytes()).unwrap();
    }

    make_executable(&mut file)?;

    Ok(path)
}

/**
 * We remove irritating info such as:
 *   - temp script file name
 *   - err line number
 *
 * The user only needs the actual error output to debug the issue.
 */
pub fn strip_line_err_info(err_output: &str) -> String {
    let re = Regex::new(r"^(.*?)line \d+:\s").unwrap();

    re.replace(err_output, String::from("")).to_string()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_creates_correct_bash_script_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path().to_str().unwrap();
        let script_file = create_script_file(
            Shell::Bash,
            vec![String::from("echo 'hello world'")],
            temp_dir_path,
        )
        .unwrap();

        assert!(script_file.contains(".sh"));

        let file = std::fs::read_to_string(script_file).unwrap();
        assert!(file.contains(BASH_STR));
        assert!(file.contains("echo 'hello world'"));
    }

    #[test]
    fn it_creates_correct_zsh_script_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path().to_str().unwrap();
        let script_file = create_script_file(
            Shell::Zsh,
            vec![String::from("echo 'hello world'")],
            temp_dir_path,
        )
        .unwrap();

        assert!(script_file.contains(".sh"));

        let file = std::fs::read_to_string(script_file).unwrap();
        assert!(file.contains(ZSH_STR));
        assert!(file.contains("echo 'hello world'"));
    }

    #[test]
    fn it_replaces_unneeded_err_info() {
        assert_eq!(
            strip_line_err_info("/home/test/temp.sh: line 5: some important info"),
            String::from("some important info")
        );
    }
}
