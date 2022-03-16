use yaml_rust::{yaml::Hash, Yaml};

use crate::commands::copy::CopyDirCommand;

pub trait CommandInterface {
    fn execute(&self, args: Hash) -> Result<(), String>;
}

pub fn validate_args(args: Hash, required: Vec<String>) -> Result<(), String> {
    for arg in required {
        if !args.contains_key(&Yaml::String(arg.clone())) {
            return Err(format!("Missing required argument: {}", arg));
        }
    }

    return Ok(());
}

pub fn get_command(name: &str) -> Result<Box<dyn CommandInterface>, String> {
    match name {
        "copy" => Ok(Box::new(CopyDirCommand {})),
        _ => Err(format!("Unknown command: {}", name)),
    }
}

// --- tests ---

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_fails_when_required_args_are_missing() {
        let mut args = Hash::new();
        args.insert(
            Yaml::String("key".to_string()),
            Yaml::String("value".to_string()),
        );
        let required = vec!["test".to_string()];

        assert!(validate_args(args, required)
            .unwrap_err()
            .contains("Missing required argument"));
    }

    #[test]
    fn it_returns_ok_when_required_args_are_present() {
        let mut args = Hash::new();
        args.insert(
            Yaml::String("key".to_string()),
            Yaml::String("value".to_string()),
        );
        let required = vec!["key".to_string()];

        assert!(validate_args(args, required).is_ok());
    }
}
