use std::path::Path;

use crate::config::base_config::*;

pub struct YamlConfig {}

impl BaseConfig for YamlConfig {
    fn read(&self, path: &str) -> Result<(), String> {
        println!("Reading yaml config from {}", path);

        // check if file exists
        if !Path::new(path).exists() {
            return Err(format!("File {} does not exist", path));
        }

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
}
