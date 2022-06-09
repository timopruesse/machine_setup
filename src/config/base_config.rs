use ergo_fs::Path;

use crate::utils::shell::Shell;

use super::{
    config_value::ConfigValue,
    json_config::{JsonConfig, ALLOWED_JSON_EXTENSIONS},
    yaml_config::{YamlConfig, ALLOWED_YAML_EXTENSIONS},
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

fn get_valid_file_endings() -> Vec<&'static str> {
    ALLOWED_YAML_EXTENSIONS
        .iter()
        .chain(ALLOWED_JSON_EXTENSIONS.iter())
        .cloned()
        .collect()
}

fn is_valid_file_ending(file_ending: &str) -> bool {
    let priorities = get_valid_file_endings();

    priorities.contains(&file_ending)
}

fn find_config_file(config_path: &str) -> Result<String, String> {
    let priorities = get_valid_file_endings();

    for ending in priorities {
        let file_path = [config_path, ending].join(".");
        if Path::new(&file_path).exists() {
            return Ok(file_path);
        }
    }

    Err(format!(
        ".{} is not a supported config file type.",
        get_file_ending(config_path)
    ))
}

fn get_config_handler(file_ending: &str) -> Result<Box<dyn BaseConfig>, String> {
    match file_ending {
        file_ending if ALLOWED_YAML_EXTENSIONS.contains(&file_ending) => {
            Ok(Box::new(YamlConfig {}))
        }
        "json" => Ok(Box::new(JsonConfig {})),
        _ => Err(format!("Unsupported config file type: {}", file_ending)),
    }
}

fn get_file_ending(path: &str) -> String {
    path.split('.').last().unwrap().to_owned()
}

pub fn get_config(config_path: &str) -> Result<TaskList, String> {
    let mut file_path = config_path.to_owned();
    let mut file_ending = get_file_ending(config_path);

    if !is_valid_file_ending(&file_ending) {
        let config_file = find_config_file(config_path);

        if let Err(config_err) = config_file {
            return Err(config_err);
        }
        file_path = config_file.unwrap();
        file_ending = get_file_ending(&file_path);
    }

    let config = get_config_handler(&file_ending);
    if config.is_err() {
        return Err(config.err().unwrap());
    }
    let config = config.unwrap();

    let result = config.read(&file_path);

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(result.unwrap())
}

#[cfg(test)]
mod test {
    use ergo_fs::IoWrite;
    use std::fs::File;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn it_finds_a_valid_config_file() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.yaml");
        let mut src_file = File::create(&src_path).unwrap();

        src_file.write_all(b"tasks:").unwrap();

        let config = get_config(src_path.to_str().unwrap());

        assert!(config.is_err());
        assert!(config.unwrap_err().contains("No tasks defined"));
    }

    #[test]
    fn it_fails_if_no_valid_config_file_is_found() {
        let config = get_config("invalid.js");

        assert!(config.is_err());
        assert!(config
            .unwrap_err()
            .contains(".js is not a supported config file type."));
    }
}
