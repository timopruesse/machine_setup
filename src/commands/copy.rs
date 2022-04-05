use ansi_term::Color::{Red, White, Yellow};
use ergo_fs::{Path, PathArc};
use std::{collections::HashMap, fs};
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    config::{validation_rules::required::Required, validator::validate_named_args},
    utils::{
        directory::{expand_path, get_source_and_target, walk_files, DIR_TARGET},
        shell::Shell,
    },
};

pub struct CopyDirCommand {}

impl CommandInterface for CopyDirCommand {
    fn install(&self, args: Yaml, _temp_dir: &str, _default_shell: &Shell) -> Result<(), String> {
        let dirs = get_source_and_target(args);
        if let Err(err_dirs) = dirs {
            return Err(err_dirs);
        }
        let dirs = dirs.unwrap();

        let result = copy_dir(&dirs.src, &dirs.target, dirs.ignore);
        if let Err(err_result) = result {
            return Err(err_result);
        }

        Ok(())
    }

    fn uninstall(&self, args: Yaml, _temp_dir: &str, _default_shell: &Shell) -> Result<(), String> {
        let validation = validate_named_args(
            args.to_owned(),
            HashMap::from([(String::from(DIR_TARGET), vec![&Required {}])]),
        );

        if let Err(err_validation) = validation {
            return Err(format!("{}", Red.paint(err_validation)));
        }

        let target_dir = args
            .as_hash()
            .unwrap()
            .get(&Yaml::String(String::from(DIR_TARGET)))
            .unwrap()
            .as_str()
            .unwrap();

        let result = remove_dir(target_dir);
        if let Err(err_result) = result {
            return Err(format!("{}", Red.paint(err_result)));
        }

        Ok(())
    }

    fn update(&self, _args: Yaml, _temp_dir: &str, _default_shell: &Shell) -> Result<(), String> {
        println!(
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
    ignore: Vec<Yaml>,
) -> Result<(), String> {
    println!(
        "Copying files from {} to {} ...",
        White.bold().paint(source_dir.to_string()),
        White.bold().paint(destination_dir.to_str().unwrap())
    );

    let result = walk_files(source_dir, destination_dir, ignore, |src, target| {
        if target_file_is_newer(src, target) {
            eprintln!(
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

        println!(
            "Copying {} to {} ...",
            White.bold().paint(src.to_str().unwrap()),
            White.bold().paint(target.to_str().unwrap())
        );

        fs::copy(src, target)
            .map_err(|e| format!("Failed to copy file: {}", Red.paint(e.to_string())))
            .ok();
    });

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(())
}

pub fn copy_dir(source: &str, destination: &str, ignore: Vec<Yaml>) -> Result<(), String> {
    let expanded_source = expand_path(source, false);
    if let Err(err_expand_src) = expanded_source {
        return Err(err_expand_src);
    }
    let source_dir = expanded_source.to_owned().unwrap();

    let expanded_destination = expand_path(destination, true);
    if let Err(err_expand_dest) = expanded_destination {
        return Err(err_expand_dest);
    }
    let destination_dir = expanded_destination.to_owned().unwrap();

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "{} {}",
            Red.paint("Source and destination directories are the same:"),
            Red.paint(source)
        ));
    }

    copy_files(&source_dir, &destination_dir, ignore)
}

pub fn remove_dir(target: &str) -> Result<(), String> {
    let expanded_target_dir = expand_path(target, false);
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

// -- tests --

#[cfg(test)]
mod test {
    use super::*;
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

        assert!(copy_dir(src, dest, vec![]).is_ok());

        let dest_file = dest_dir.path().join(src_file.path().file_name().unwrap());

        assert!(dest_file.exists());
    }

    #[test]
    fn it_removes_dir() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        assert!(remove_dir(path).is_ok());
        assert!(!dir.path().exists());
    }
}
