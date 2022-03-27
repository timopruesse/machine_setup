use std::collections::HashMap;

use ergo_fs::PathDir;
use git_commands::git;
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    config::{validation_rules::required::Required, validator::validate_args},
    utils::directory::expand_dir,
};

pub struct CloneCommand {}

impl CommandInterface for CloneCommand {
    fn install(&self, args: Yaml) -> Result<(), String> {
        let rules = vec![&Required {}];

        let validation = validate_args(
            args.to_owned(),
            HashMap::from([
                (String::from("url"), rules.clone()),
                (String::from("target"), rules.clone()),
            ]),
        );
        if validation.is_err() {
            return Err(validation.unwrap_err());
        }

        let url = args
            .as_hash()
            .unwrap()
            .get(&Yaml::String(String::from("url")))
            .unwrap()
            .as_str()
            .unwrap();

        let target = args
            .as_hash()
            .unwrap()
            .get(&Yaml::String(String::from("target")))
            .unwrap()
            .as_str()
            .unwrap();

        let expanded_target_dir = expand_dir(target, true);
        if expanded_target_dir.is_err() {
            return Err(expanded_target_dir.unwrap_err());
        }
        let expanded_target_dir = expanded_target_dir.unwrap();

        let result = clone_repository(url, &expanded_target_dir);
        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }

    fn uninstall(&self, args: Yaml) -> Result<(), String> {
        let validation = validate_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), vec![&Required {}])]),
        );

        if validation.is_err() {
            return Err(validation.unwrap_err());
        }

        let target = args
            .as_hash()
            .unwrap()
            .get(&Yaml::String(String::from("target")))
            .unwrap()
            .as_str()
            .unwrap();

        let expanded_target_dir = expand_dir(target, false);
        if expanded_target_dir.is_err() {
            return Err(expanded_target_dir.unwrap_err());
        }
        let expanded_target_dir = expanded_target_dir.unwrap();

        let result = remove_repository(&expanded_target_dir);
        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }

    fn update(&self, args: Yaml) -> Result<(), String> {
        let validation = validate_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), vec![&Required {}])]),
        );

        if validation.is_err() {
            return Err(validation.unwrap_err());
        }

        let target = args
            .as_hash()
            .unwrap()
            .get(&Yaml::String(String::from("target")))
            .unwrap()
            .as_str()
            .unwrap();

        let expanded_target_dir = expand_dir(target, false);
        if expanded_target_dir.is_err() {
            return Err(expanded_target_dir.unwrap_err());
        }
        let expanded_target_dir = expanded_target_dir.unwrap();

        let result = update_repository(&expanded_target_dir);
        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }
}

pub fn clone_repository(url: &str, target: &PathDir) -> Result<(), String> {
    println!("Cloning {} into {} ...", url, target.to_str().unwrap());

    let clone_result = git(&["clone", url, "."], target);
    if clone_result.is_err() {
        return Err(clone_result.unwrap_err().to_string());
    }

    return Ok(());
}

pub fn remove_repository(target: &PathDir) -> Result<(), String> {
    println!("Removing {} ...", target.to_str().unwrap());

    let remove_result = std::fs::remove_dir_all(target);
    if remove_result.is_err() {
        return Err(remove_result.unwrap_err().to_string());
    }

    return Ok(());
}

pub fn update_repository(target: &PathDir) -> Result<(), String> {
    println!("Updating {} ...", target.to_str().unwrap());

    let update_result = git(&["pull"], target);
    if update_result.is_err() {
        return Err(update_result.unwrap_err().to_string());
    }

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

    #[test]
    fn test_remove_repository() {
        unimplemented!()
    }

    #[test]
    fn test_update_repository() {
        unimplemented!()
    }
}
