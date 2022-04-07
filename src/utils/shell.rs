use core::fmt;
use std::{fs::Permissions, os::unix::prelude::PermissionsExt, str::FromStr};

use ergo_fs::IoWrite;

use super::temp_storage::create_temp_file;

#[derive(Debug)]
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
            _ => Err(format!("Unknown shell: {}", s)),
        }
    }
}

const BASH_STR: &str = "#!/bin/bash\nsource $HOME/.bashrc\n";
const ZSH_STR: &str = "#!/bin/zsh\nsource $HOME/.zshrc\n";

pub fn create_script_file(
    shell: Shell,
    commands: Vec<String>,
    temp_dir: &str,
) -> Result<String, String> {
    let temp_file = create_temp_file("sh", temp_dir);
    if let Err(err_temp_file) = temp_file {
        return Err(err_temp_file);
    }
    let temp_file = temp_file.unwrap();
    let mut file = temp_file.file;
    let path = temp_file.path;

    match shell {
        Shell::Bash => file.write_all(String::from(BASH_STR).as_bytes()).unwrap(),
        Shell::Zsh => file.write_all(String::from(ZSH_STR).as_bytes()).unwrap(),
    }

    for command in commands {
        file.write_all(format!("{}\n", command).as_bytes()).unwrap();
    }

    let perm_result = file.set_permissions(Permissions::from_mode(0o755));
    if let Err(err_perm) = perm_result {
        return Err(err_perm.to_string());
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
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
}
