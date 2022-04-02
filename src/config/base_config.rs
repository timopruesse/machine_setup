use yaml_rust::Yaml;

use super::yaml_config::YamlConfig;

#[derive(Debug)]
pub struct Command {
    pub name: String,
    pub args: Yaml,
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub commands: Vec<Command>,
}

#[derive(Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

pub trait BaseConfig {
    fn read(&self, path: &str) -> Result<TaskList, String>;
}

fn get_config_handler(config_path: &str) -> Result<Box<dyn BaseConfig>, String> {
    let file_ending = config_path.split('.').last().unwrap();

    return match file_ending {
        file_ending if file_ending == "yml" || file_ending == "yaml" => Ok(Box::new(YamlConfig {})),
        _ => Err(format!("Unsupported config file format: .{}", file_ending)),
    };
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
