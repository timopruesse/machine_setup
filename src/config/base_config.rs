use crate::utils::shell::Shell;

use super::{
    config_value::ConfigValue,
    json_config::JsonConfig,
    yaml_config::{YamlConfig, ALLOWED_EXTENSIONS},
};

#[derive(Debug)]
pub struct Command {
    pub name: String,
    pub args: ConfigValue,
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub commands: Vec<Command>,
}

#[derive(Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
    pub temp_dir: String,
    pub default_shell: Shell,
}

pub trait BaseConfig {
    fn read(&self, path: &str) -> Result<TaskList, String>;
}

fn get_config_handler(config_path: &str) -> Result<Box<dyn BaseConfig>, String> {
    let file_ending = config_path.split('.').last().unwrap();

    match file_ending {
        file_ending if ALLOWED_EXTENSIONS.contains(&file_ending) => Ok(Box::new(YamlConfig {})),
        "json" => Ok(Box::new(JsonConfig {})),
        _ => Err(format!("Unsupported config file type: {}", file_ending)),
    }
}

pub fn get_config(config_path: &str) -> Result<TaskList, String> {
    let config = get_config_handler(config_path);
    if config.is_err() {
        return Err(config.err().unwrap());
    }
    let config = config.unwrap();

    let result = config.read(config_path);

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(result.unwrap())
}
