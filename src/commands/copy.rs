use ansi_term::Color::{Red, White, Yellow};
use ergo_fs::{Path, PathArc};
use indicatif::ProgressBar;
use std::{
    collections::HashMap,
    fs::{self, canonicalize},
};
use tracing::{debug, warn};

use crate::{
    command::{CommandConfig, CommandInterface},
    config::{
        config_value::ConfigValue,
        validation_rules::required::Required,
        validator::{validate_named_args, ValidationRule},
    },
    utils::directory::{
        expand_path, get_relative_dir, get_source_and_target, walk_files, DIR_TARGET,
    },
};

pub struct CopyDirCommand {}

impl CommandInterface for CopyDirCommand {
    fn install(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let dirs = get_source_and_target(args, &config.config_dir)?;

        copy_dir(&dirs.src, &dirs.target, dirs.ignore, progress)
    }

    fn uninstall(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        _progress: &ProgressBar,
    ) -> Result<(), String> {
        let rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];

        validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from(DIR_TARGET), rules)]),
        )?;

        let target_dir = args
            .as_hash()
            .unwrap()
            .get(DIR_TARGET)
            .unwrap()
            .as_str()
            .unwrap();

        let relative_target_path = get_relative_dir(&config.config_dir, target_dir);
        let abs_target_path = canonicalize(relative_target_path);
        if let Err(target_err) = abs_target_path {
            if target_err.raw_os_error().unwrap() == 2 {
                warn!("{}", Yellow.paint("The file(s) were already removed..."));
                return Ok(());
            }

            return Err(format!("{}", target_err));
        }
        let abs_target_path = abs_target_path.unwrap();

        if abs_target_path.as_os_str() == config.config_dir.as_os_str() {
            return Err(format!("{}", Red.paint("cannot delete config_dir")));
        }

        remove_dir(&abs_target_path)
    }

    fn update(
        &self,
        _args: ConfigValue,
        _config: &CommandConfig,
        _progress: &ProgressBar,
    ) -> Result<(), String> {
        warn!(
            "{}",
            Yellow.paint("update not implemented for copy command")
        );
        Ok(())
    }
}

fn target_file_is_newer(file_src: &Path, file_target: &Path) -> bool {
    if !file_target.exists() {
        return false;
    }

    let file_src_meta = file_src.metadata().unwrap();
    let file_target_meta = file_target.metadata().unwrap();

    file_target_meta.modified().unwrap() > file_src_meta.modified().unwrap()
}

fn copy_files(
    source_dir: &PathArc,
    destination_dir: &Path,
    ignore: Vec<ConfigValue>,
    progress: &ProgressBar,
) -> Result<(), String> {
    let message = format!(
        "Copying files from {} to {} ...",
        White.bold().paint(source_dir.to_string()),
        White.bold().paint(destination_dir.to_str().unwrap())
    );

    debug!(message);
    progress.set_message(message);

    walk_files(source_dir, destination_dir, ignore, |src, target| {
        if target_file_is_newer(src, target) {
            warn!(
                "{} {}: {}",
                Yellow.paint("! Skipping !"),
                Yellow
                    .bold()
                    .paint(target.file_name().unwrap().to_str().unwrap()),
                Yellow
                    .bold()
                    .paint("The target file is newer than the source file.")
            );
            return;
        }

        debug!(
            "Copying {} to {} ...",
            White.bold().paint(src.to_str().unwrap()),
            White.bold().paint(target.to_str().unwrap())
        );

        fs::copy(src, target)
            .map_err(|e| format!("Failed to copy file: {}", Red.paint(e.to_string())))
            .ok();
    })
}

pub fn copy_dir(
    source: &str,
    destination: &str,
    ignore: Vec<ConfigValue>,
    progress: &ProgressBar,
) -> Result<(), String> {
    let source_dir = expand_path(source, false)?;
    let destination_dir = expand_path(destination, true)?;

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "{} {}",
            Red.paint("Source and destination directories are the same:"),
            Red.paint(source)
        ));
    }

    copy_files(&source_dir, &destination_dir, ignore, progress)
}

pub fn remove_dir(target: &Path) -> Result<(), String> {
    let expanded_target_dir = expand_path(target.to_str().unwrap(), false);
    if expanded_target_dir.is_err() {
        return Err(expanded_target_dir.err().unwrap());
    }
    let expanded_target_dir = expanded_target_dir.unwrap();

    let result = fs::remove_dir_all(expanded_target_dir);

    if result.is_err() {
        return Err(result.err().unwrap().to_string());
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::utils::shell::Shell;

    use super::*;
    use ergo_fs::PathDir;
    use tempfile::{tempdir, tempfile_in, NamedTempFile};

    #[test]
    fn it_fails_when_dirs_are_the_same() {
        let dir = tempdir().unwrap();
        let src_path = dir.path();
        tempfile_in(&src_path).unwrap();
        let src = src_path.to_str().unwrap();

        assert!(copy_dir(src, src, vec![])
            .unwrap_err()
            .contains("Source and destination directories are the same"));
    }

    #[test]
    fn it_copies_files() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path();
        let src_file = NamedTempFile::new_in(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        copy_dir(src, dest, vec![]).unwrap();

        let dest_file = dest_dir.path().join(src_file.path().file_name().unwrap());

        assert!(dest_file.exists());
    }

    #[test]
    fn it_removes_dir() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        remove_dir(&PathArc::new(path)).unwrap();
        assert!(!dir.path().exists());
    }

    #[test]
    fn it_doesnt_remove_config_dir() {
        let copy = CopyDirCommand {};

        let dir = tempdir().unwrap();
        let config_dir = PathDir::new(&dir).unwrap();

        let mut args = HashMap::new();
        args.insert(
            String::from("source"),
            ConfigValue::String(String::from("./test")),
        );
        args.insert(
            String::from("target"),
            ConfigValue::String(String::from(".")),
        );

        let result = copy.uninstall(
            ConfigValue::Hash(args),
            &CommandConfig {
                config_dir,
                temp_dir: tempdir().unwrap().path().to_str().unwrap().to_string(),
                default_shell: Shell::Bash,
            },
        );

        assert!(dir.path().exists());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot delete config_dir"))
    }
}
