extern crate yaml_rust;

use yaml_rust::{Yaml, YamlLoader};

use crate::config::base_config::*;
use std::{io::Read, path::Path};

#[derive(Debug)]
pub struct YamlConfig {}

static ALLOWED_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

fn parse_yaml(path: &Path) -> Result<TaskList, String> {
    let mut file = std::fs::File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let config = YamlLoader::load_from_str(&contents).unwrap();

    let entries = &config[0];

    if entries["tasks"] == Yaml::BadValue {
        return Err(String::from("no tasks found"));
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
                args: args.to_owned(),
            });
        }

        let task = Task {
            name: key.as_str().unwrap().to_string(),
            commands,
        };
        tasks.push(task);
    }

    Ok(TaskList { tasks })
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

        println!("Reading config from {} ...", path);

        parse_yaml(yaml_path)
    }
}

// -- tests --

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

        assert!(result.unwrap_err().contains("no tasks found"));
    }
}
