use std::path::Path;

use crate::config::base_config::*;

pub struct YamlConfig {}

static ALLOWED_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

impl BaseConfig for YamlConfig {
    fn read(&self, path: &str) -> Result<(), String> {
        let yaml_path = Path::new(path);

        if !yaml_path.exists() {
            return Err(format!("File {} does not exist", path));
        }

        if !ALLOWED_EXTENSIONS.contains(&yaml_path.extension().unwrap().to_str().unwrap()) {
            return Err(format!("File {} is not a yaml file", path));
        }

        println!("Reading yaml config from {}", path);

        return Ok(());
    }

    fn next_task(&self) -> Option<Task> {
        println!("Getting next task from yaml config");

        Some(Task {
            name: "Test task".to_string(),
            args: vec!["test".to_string()],
        })
    }
}

// -- tests --

#[cfg(test)]
mod test {
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
}
