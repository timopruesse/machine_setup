use ansi_term::Color::White;
use serde_json::Value;
use tracing::info;

use crate::{
    config::base_config::*,
    task::Task,
    utils::{shell::Shell, threads::get_thread_number},
};
use std::{collections::HashMap, io::Read, path::Path, str::FromStr};

use super::{config_value::ConfigValue, os::Os};

#[derive(Debug)]
pub struct JsonConfig {}

pub static ALLOWED_JSON_EXTENSIONS: [&str; 1] = ["json"];

fn convert_to_config_value(json: &Value) -> ConfigValue {
    match json {
        Value::String(s) => ConfigValue::String(s.to_string()),
        Value::Number(n) => ConfigValue::Integer(n.as_i64().unwrap() as i32),
        Value::Bool(b) => ConfigValue::Boolean(b.to_owned()),
        Value::Array(a) => {
            let mut array = Vec::new();
            for json in a {
                array.push(convert_to_config_value(json));
            }
            ConfigValue::Array(array)
        }
        Value::Object(o) => {
            let mut hash = HashMap::new();
            for (key, value) in o {
                hash.insert(key.to_string(), convert_to_config_value(value));
            }
            ConfigValue::Hash(hash)
        }
        Value::Null => ConfigValue::Null,
    }
}

fn get_os_list(value: &Value) -> Result<Vec<Os>, String> {
    if value.is_null() {
        return Ok(vec![]);
    }

    if value.is_array() {
        return Ok(value
            .as_array()
            .unwrap()
            .iter()
            .map(|os| Os::from_str(os.as_str().unwrap()).unwrap())
            .collect());
    }

    if value.is_string() {
        return Ok(vec![Os::from_str(value.as_str().unwrap()).unwrap()]);
    }

    Err(format!("{value:?} is in the wrong format"))
}

fn get_commands(value: &Value) -> Result<Vec<Command>, String> {
    if value.is_null() {
        return Err(String::from("No commands defined"));
    }

    if !value.is_array() {
        return Err(String::from("Commands have to be a list"));
    }

    let mut commands: Vec<Command> = vec![];
    for command in value.as_array().unwrap().iter() {
        if !command.is_object() {
            return Err(String::from("command definition is incorrect"));
        }

        let command_map = command.as_object().unwrap();
        for c in command_map.into_iter() {
            let (name, args) = c;

            commands.push(Command {
                name: name.to_string(),
                args: convert_to_config_value(args),
            });
        }
    }

    Ok(commands)
}

fn parse_json(path: &Path) -> Result<TaskList, String> {
    let mut file = std::fs::File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let config: Result<Value, serde_json::Error> = serde_json::from_str(&contents);
    if let Err(config_err) = config {
        return Err(format!("{config_err}"));
    }
    let config = config.unwrap();

    if config["tasks"] == Value::Null {
        return Err(String::from("\nNo tasks defined"));
    }

    let mut tasks: Vec<Task> = vec![];

    for task in config["tasks"].as_object().unwrap().iter() {
        let (key, value) = task;

        if !value.is_object() {
            return Err(format!("{key}: task definition is incorrect"));
        }

        let values = value.as_object().unwrap();

        let commands = get_commands(&values["commands"])?;
        let os_list = get_os_list(values.get("os").unwrap_or(&Value::Null))?;

        let task = Task {
            name: key.to_string(),
            os: os_list,
            commands,
            parallel: values
                .get("parallel")
                .unwrap_or(&Value::Bool(false))
                .as_bool()
                .unwrap(),
        };
        tasks.push(task);
    }

    let temp_dir = config["temp_dir"]
        .as_str()
        .unwrap_or("~/.machine_setup")
        .to_string();

    let default_shell_str = config["default_shell"]
        .as_str()
        .unwrap_or("bash")
        .to_string();

    let default_shell = Shell::from_str(&default_shell_str);
    if let Err(err_shell) = default_shell {
        return Err(format!("default_shell: {err_shell}"));
    }
    let default_shell = default_shell.unwrap();

    let parallel = config["parallel"].as_bool().unwrap_or(false);

    Ok(TaskList {
        tasks,
        temp_dir,
        default_shell,
        num_threads: get_thread_number(config["num_threads"].as_i64()),
        parallel,
    })
}

impl BaseConfig for JsonConfig {
    fn read(&self, path: &str) -> Result<TaskList, String> {
        let json_path = Path::new(path);

        if !json_path.exists() {
            return Err(format!("File {path} does not exist"));
        }

        if json_path.extension().unwrap().to_str().unwrap() != "json" {
            return Err(format!("File {path} is not a JSON file"));
        }

        info!("Reading config from {} ...", White.bold().paint(path));

        parse_json(json_path)
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn it_fails_when_config_file_is_missing() {
        let config = JsonConfig {};
        let result = config.read("/tmp/missing.json");
        result.unwrap_err();
    }

    #[test]
    fn it_fails_when_config_file_is_not_json() {
        let config = JsonConfig {};
        let result = config.read("/tmp/test.txt");
        result.unwrap_err();
    }

    #[test]
    fn it_fails_when_tasks_are_not_defined() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.json");
        let mut src_file = File::create(&src_path).unwrap();

        src_file.write_all(b"{\"text\": \"hello world\"}").unwrap();

        let config = JsonConfig {};
        let result = config.read(src_path.to_str().unwrap());

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No tasks defined"));
    }

    #[test]
    fn it_fails_when_commands_are_not_a_list() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.json");
        let mut src_file = File::create(&src_path).unwrap();

        src_file
            .write_all(b"{ \"tasks\": { \"test\": { \"commands\": { \"invalid\": 0 } } } }")
            .unwrap();

        let config = JsonConfig {};
        let result = config.read(src_path.to_str().unwrap());

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Commands have to be a list"));
    }
}
