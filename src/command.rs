use yaml_rust::Yaml;

use crate::commands::{clone::CloneCommand, copy::CopyDirCommand, symlink::SymlinkCommand};

pub trait CommandInterface {
    fn install(&self, args: Yaml) -> Result<(), String>;
    fn uninstall(&self, args: Yaml) -> Result<(), String>;
    fn update(&self, args: Yaml) -> Result<(), String>;
}

pub fn get_command(name: &str) -> Result<Box<dyn CommandInterface>, String> {
    match name {
        "copy" => Ok(Box::new(CopyDirCommand {})),
        "symlink" => Ok(Box::new(SymlinkCommand {})),
        "clone" => Ok(Box::new(CloneCommand {})),
        _ => Err(format!("Unknown command: {}", name)),
    }
}
