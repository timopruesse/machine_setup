use yaml_rust::Yaml;

use crate::{
    commands::{
        clone::CloneCommand, copy::CopyDirCommand, run::RunCommand, symlink::SymlinkCommand,
    },
    utils::shell::Shell,
};

pub trait CommandInterface {
    fn install(&self, args: Yaml, temp_dir: &str, default_shell: &Shell) -> Result<(), String>;
    fn uninstall(&self, args: Yaml, temp_dir: &str, default_shell: &Shell) -> Result<(), String>;
    fn update(&self, args: Yaml, temp_dir: &str, default_shell: &Shell) -> Result<(), String>;
}

pub fn get_command(name: &str) -> Result<Box<dyn CommandInterface>, String> {
    match name {
        "copy" => Ok(Box::new(CopyDirCommand {})),
        "symlink" => Ok(Box::new(SymlinkCommand {})),
        "clone" => Ok(Box::new(CloneCommand {})),
        "run" => Ok(Box::new(RunCommand {})),
        _ => Err(format!("Unknown command: {}", name)),
    }
}
