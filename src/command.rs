use ergo_fs::PathDir;

use crate::{
    commands::{
        clone::CloneCommand, copy::CopyDirCommand, run::RunCommand, symlink::SymlinkCommand,
    },
    config::config_value::ConfigValue,
    utils::shell::Shell,
};

#[derive(Debug, Clone)]
pub struct CommandConfig {
    pub config_dir: PathDir,
    pub temp_dir: String,
    pub default_shell: Shell,
}

pub trait CommandInterface {
    fn install(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String>;
    fn uninstall(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String>;
    fn update(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String>;
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
