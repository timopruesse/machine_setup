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

pub fn create_script_file(
    shell: Shell,
    commands: Vec<String>,
    temp_dir: &str,
) -> Result<String, String> {
    let temp_file = create_temp_file("sh", temp_dir);
    if let Err(err_temp_file) = temp_file {
        return Err(err_temp_file.to_string());
    }
    let temp_file = temp_file.unwrap();
    let mut file = temp_file.file;
    let path = temp_file.path;

    match shell {
        Shell::Zsh => file
            .write_all(format!("#!/bin/zsh\nsource $HOME/.zshrc\n").as_bytes())
            .unwrap(),
        Shell::Bash => file
            .write_all(format!("#!/bin/bash\nsource $HOME/.bashrc\n").as_bytes())
            .unwrap(),
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
