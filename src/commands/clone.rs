use ansi_term::Color::{White, Yellow};
use std::collections::HashMap;
use tracing::info;

use ergo_fs::PathArc;
use git_commands::git;

use crate::{
    command::{CommandConfig, CommandInterface},
    config::{
        config_value::ConfigValue, validation_rules::required::Required,
        validator::validate_named_args,
    },
    utils::directory::{expand_path, get_relative_dir},
};

pub struct CloneCommand {}

fn get_installed_repo_url(target_dir: &PathArc) -> Result<String, String> {
    let result = git(&["config", "--get", "remote.origin.url"], target_dir);
    if let Err(err_result) = result {
        return Err(err_result.to_string());
    }

    Ok(String::from_utf8(result.unwrap().stdout)
        .unwrap()
        .trim()
        .to_string())
}

fn is_repo_installed(url: &str, target_dir: &PathArc) -> bool {
    let installed_repo_url = get_installed_repo_url(target_dir);
    if installed_repo_url.is_err() {
        return false;
    }
    let installed_repo_url = installed_repo_url.unwrap();

    installed_repo_url == url
}

impl CommandInterface for CloneCommand {
    fn install(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        let rules = vec![&Required {}];

        validate_named_args(
            args.to_owned(),
            HashMap::from([
                (String::from("url"), rules.clone()),
                (String::from("target"), rules.clone()),
            ]),
        )?;

        let url = args
            .as_hash()
            .unwrap()
            .get("url")
            .unwrap()
            .as_str()
            .unwrap();

        let target = args
            .as_hash()
            .unwrap()
            .get("target")
            .unwrap()
            .as_str()
            .unwrap();

        let relative_target_dir = get_relative_dir(&config.config_dir, target);
        let expanded_target_dir = expand_path(relative_target_dir.as_str(), true)?;

        if is_repo_installed(url, &expanded_target_dir) {
            info!(
                "{} {}",
                Yellow.paint("The repository was already cloned."),
                Yellow.bold().paint("Updating...")
            );
            return self.update(args, config);
        }

        clone_repository(url, &expanded_target_dir)
    }

    fn uninstall(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), vec![&Required {}])]),
        )?;

        let target = args
            .as_hash()
            .unwrap()
            .get("target")
            .unwrap()
            .as_str()
            .unwrap();

        let relative_target_dir = get_relative_dir(&config.config_dir, target);
        let expanded_target_dir = expand_path(relative_target_dir.as_str(), false)?;

        remove_repository(&expanded_target_dir)
    }

    fn update(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), vec![&Required {}])]),
        )?;

        let target = args
            .as_hash()
            .unwrap()
            .get("target")
            .unwrap()
            .as_str()
            .unwrap();

        let relative_target_dir = get_relative_dir(&config.config_dir, target);
        let expanded_target_dir = expand_path(relative_target_dir.as_str(), true)?;

        update_repository(&expanded_target_dir)
    }
}

pub fn clone_repository(url: &str, target: &PathArc) -> Result<(), String> {
    info!(
        "Cloning {} into {} ...",
        White.bold().paint(url),
        White.bold().paint(target.to_str().unwrap())
    );

    let clone_result = git(&["clone", url, "."], target);
    if let Err(err_clone) = clone_result {
        return Err(err_clone.to_string());
    }

    Ok(())
}

pub fn remove_repository(target: &PathArc) -> Result<(), String> {
    info!(
        "Removing {} ...",
        White.bold().paint(target.to_str().unwrap())
    );

    let remove_result = std::fs::remove_dir_all(target);
    if let Err(err_remove) = remove_result {
        return Err(err_remove.to_string());
    }

    Ok(())
}

pub fn update_repository(target: &PathArc) -> Result<(), String> {
    info!(
        "Updating {} ...",
        White.bold().paint(target.to_str().unwrap())
    );

    let update_result = git(&["pull"], target);
    if let Err(err_update) = update_result {
        return Err(err_update.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_remove_repository() {
        let target = tempfile::tempdir().unwrap();
        let target_path = target.path().to_str().unwrap();

        let result = remove_repository(&PathArc::new(target_path));
        result.unwrap();
        assert!(!target.path().exists());
    }
}
