use yaml_rust::Yaml;

use crate::command::{validate_args, CommandInterface};

pub struct CloneCommand {}

impl CommandInterface for CloneCommand {
    fn install(&self, args: yaml_rust::yaml::Hash) -> Result<(), String> {
        let validation = validate_args(
            args.to_owned(),
            vec![String::from("url"), String::from("target")],
        );
        if validation.is_err() {
            return Err(validation.unwrap_err());
        }

        let url = args
            .get(&Yaml::String(String::from("url")))
            .unwrap()
            .as_str()
            .unwrap();

        let target = args
            .get(&Yaml::String(String::from("target")))
            .unwrap()
            .as_str()
            .unwrap();

        let result = clone_repository(url, target);
        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }

    fn uninstall(&self, args: yaml_rust::yaml::Hash) -> Result<(), String> {
        unimplemented!()
    }

    fn update(&self, args: yaml_rust::yaml::Hash) -> Result<(), String> {
        unimplemented!()
    }
}

pub fn clone_repository(url: &str, target: &str) -> Result<(), String> {
    println!("Cloning {} into {} ...", url, target);

    return Ok(());
}

// -- tests --

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_clone_repository() {
        unimplemented!()
    }
}
