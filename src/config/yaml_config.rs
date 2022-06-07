extern crate yaml_rust;

use ansi_term::Color::White;
use yaml_rust::{Yaml, YamlLoader};

use crate::{config::base_config::*, utils::shell::Shell};
use std::{collections::HashMap, io::Read, path::Path, str::FromStr};

use super::config::ConfigValue;

#[derive(Debug)]
pub struct YamlConfig {}

static ALLOWED_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

fn convert_to_config_value(yaml: &Yaml) -> ConfigValue {
    match yaml {
        Yaml::String(s) => ConfigValue::String(s.to_string()),
        Yaml::Integer(i) => ConfigValue::Integer(i.to_owned() as i32),
        Yaml::Real(f) => ConfigValue::Float(f.parse().unwrap()),
        Yaml::Boolean(b) => ConfigValue::Boolean(b.to_owned()),
        Yaml::Array(a) => {
            let mut array = Vec::new();
            for yaml in a {
                array.push(convert_to_config_value(yaml));
            }
            ConfigValue::Array(array)
        }
        Yaml::Hash(h) => {
            let mut hash = HashMap::new();
            for (key, value) in h {
                hash.insert(
                    key.as_str().unwrap().to_string(),
                    convert_to_config_value(value),
                );
            }
            ConfigValue::Hash(hash)
        }
        Yaml::Null => ConfigValue::Null,
        Yaml::BadValue => ConfigValue::Invalid,
        _ => ConfigValue::Invalid,
    }
}

fn parse_yaml(path: &Path) -> Result<TaskList, String> {
    let mut file = std::fs::File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let config = YamlLoader::load_from_str(&contents).unwrap();

    let entries = &config[0];
    if entries["tasks"] == Yaml::BadValue || entries["tasks"] == Yaml::Null {
        return Err(String::from("\nNo tasks defined"));
    }

    let mut tasks: Vec<Task> = vec![];
    for task in entries["tasks"].as_hash().unwrap().iter() {
        let (key, value) = task;

        if value.clone().into_hash().is_none() {
            return Err(format!(
                "{}: task definition is incorrect",
                key.as_str().unwrap()
            ));
        }

        let mut commands: Vec<Command> = vec![];
        for command in value.as_hash().unwrap().iter() {
            let (name, args) = command;

            commands.push(Command {
                name: name.as_str().unwrap().to_string(),
                args: convert_to_config_value(args),
            });
        }

        let task = Task {
            name: key.as_str().unwrap().to_string(),
            commands,
        };
        tasks.push(task);
    }

    let temp_dir = entries["temp_dir"]
        .as_str()
        .unwrap_or("~/.machine_setup")
        .to_string();

    let default_shell_str = entries["default_shell"]
        .as_str()
        .unwrap_or("bash")
        .to_string();

    let default_shell = Shell::from_str(&default_shell_str);
    if let Err(err_shell) = default_shell {
        return Err(format!("default_shell: {}", err_shell));
    }
    let default_shell = default_shell.unwrap();

    Ok(TaskList {
        tasks,
        temp_dir,
        default_shell,
    })
}

impl BaseConfig for YamlConfig {
    fn read(&self, path: &str) -> Result<TaskList, String> {
        let yaml_path = Path::new(path);

        if !yaml_path.exists() {
            return Err(format!("File {} does not exist", path));
        }

        if !ALLOWED_EXTENSIONS.contains(&yaml_path.extension().unwrap().to_str().unwrap()) {
            return Err(format!("File {} is not a yaml file", path));
        }

        println!("Reading config from {} ...", White.bold().paint(path));

        parse_yaml(yaml_path)
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn it_fails_when_config_file_is_missing() {
        let config = YamlConfig {};
        let result = config.read("/tmp/missing.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn it_fails_when_config_file_is_not_yaml() {
        let config = YamlConfig {};
        let result = config.read("/tmp/test.txt");
        assert!(result.is_err());
    }

    #[test]
    fn it_fails_when_tasks_are_not_defined() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.yaml");
        let mut src_file = File::create(&src_path).unwrap();
        // write string to src_file
        src_file.write_all(b"text: hello world").unwrap();

        let config = YamlConfig {};
        let result = config.read(src_path.to_str().unwrap());

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No tasks defined"));
    }
}
