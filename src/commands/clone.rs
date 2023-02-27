use ansi_term::Color::{White, Yellow};
use indicatif::ProgressBar;
use std::collections::HashMap;
use tracing::{debug, info};

use ergo_fs::{Path, PathBuf};
use git_commands::git;

use crate::{
    command::{CommandConfig, CommandInterface},
    config::{
        config_value::ConfigValue,
        validation_rules::required::Required,
        validator::{validate_named_args, ValidationRule},
    },
    utils::directory::{expand_path, get_relative_dir},
};

pub struct CloneCommand {}

fn get_installed_repo_url(target_dir: &Path) -> Result<PathBuf, String> {
    let output = git(&["config", "--get", "remote.origin.url"], target_dir)
        .map_err(|e| e.to_string())?
        .stdout;

    let url = String::from_utf8_lossy(&output).trim().to_string();

    Ok(PathBuf::from(url))
}

fn is_repo_installed(url: &str, target_dir: &Path) -> bool {
    if let Ok(installed_repo_url) = get_installed_repo_url(target_dir).map(PathBuf::into_os_string)
    {
        url.eq(&installed_repo_url)
    } else {
        false
    }
}

impl CommandInterface for CloneCommand {
    fn install(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let url_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];
        let target_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];

        validate_named_args(
            args.to_owned(),
            HashMap::from([
                (String::from("url"), url_rules),
                (String::from("target"), target_rules),
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
            return self.update(args, config, progress);
        }

        clone_repository(url, &expanded_target_dir, progress)
    }

    fn uninstall(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];

        validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), rules)]),
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

        remove_repository(&expanded_target_dir, progress)
    }

    fn update(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];

        validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from("target"), rules)]),
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

        update_repository(&expanded_target_dir, progress)
    }
}

pub fn clone_repository(url: &str, target: &Path, progress: &ProgressBar) -> Result<(), String> {
    let message = format!(
        "Cloning {} into {} ...",
        White.bold().paint(url),
        White.bold().paint(target.display().to_string())
    );

    debug!(message);
    progress.set_message(message);

    let clone_result = git(&["clone", url, "."], target);
    if let Err(err_clone) = clone_result {
        return Err(err_clone.to_string());
    }

    Ok(())
}

pub fn remove_repository(target: &PathBuf, progress: &ProgressBar) -> Result<(), String> {
    let message = format!(
        "Removing {} ...",
        White.bold().paint(target.display().to_string())
    );

    debug!(message);
    progress.set_message(message);

    std::fs::remove_dir_all(target).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_repository(target: &Path, progress: &ProgressBar) -> Result<(), String> {
    let message = format!(
        "Updating {} ...",
        White.bold().paint(target.display().to_string())
    );

    debug!(message);
    progress.set_message(message);

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

        let pb = ProgressBar::new(0);

        let result = remove_repository(&PathBuf::from(target_path), &pb);
        result.unwrap();
        assert!(!target.path().exists());
    }
}
