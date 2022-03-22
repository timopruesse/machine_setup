use symlink::symlink_dir;
use yaml_rust::{yaml::Hash, Yaml};

use crate::{
    command::{validate_args, CommandInterface},
    utils::directory::expand_dir,
};

pub struct SymlinkCommand {}

static SYMLINK_DIR_SRC: &str = "src";
static SYMLINK_DIR_TARGET: &str = "target";

impl CommandInterface for SymlinkCommand {
    fn install(&self, args: Hash) -> Result<(), String> {
        let validation = validate_args(
            args.to_owned(),
            vec![
                String::from(SYMLINK_DIR_SRC),
                String::from(SYMLINK_DIR_TARGET),
            ],
        );
        if validation.is_err() {
            return Err(validation.unwrap_err());
        }

        let src_dir = args
            .get(&Yaml::String(String::from(SYMLINK_DIR_SRC)))
            .unwrap()
            .as_str()
            .unwrap();

        let target_dir = args
            .get(&Yaml::String(String::from(SYMLINK_DIR_TARGET)))
            .unwrap()
            .as_str()
            .unwrap();

        if target_dir.is_empty() {
            return Err(String::from("Target directory cannot be empty"));
        }

        let result = create_symlink(src_dir, target_dir);

        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }

    fn uninstall(&self, args: Hash) -> Result<(), String> {
        // TODO
        return Ok(());
    }

    fn update(&self, args: Hash) -> Result<(), String> {
        // TODO
        return Ok(());
    }
}

pub fn create_symlink(source: &str, destination: &str) -> Result<(), String> {
    let expanded_source = expand_dir(source, false);
    if expanded_source.is_err() {
        return Err(expanded_source.unwrap_err().to_string());
    }
    let source_dir = expanded_source.to_owned().unwrap();

    let expanded_destination = expand_dir(destination, true);
    if expanded_destination.is_err() {
        return Err(expanded_destination.unwrap_err().to_string());
    }
    let destination_dir = expanded_destination.to_owned().unwrap();

    if !source_dir.exists() {
        return Err(format!("Source directory does not exist: {}", source));
    }

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "Source and destination directories are the same: {}",
            source
        ));
    }

    println!("Symlinking {} to {}", source, destination);

    let result = symlink_dir(destination_dir, source_dir);

    if result.is_err() {
        return Err(result.unwrap_err().to_string());
    }

    return Ok(());
}

// -- tests --

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn it_fails_when_src_dir_doesnt_exist() {
        assert!(create_symlink("invalid", "invalid")
            .unwrap_err()
            .contains("Source directory does not exist"));
    }

    #[test]
    fn it_fails_when_dirs_are_the_same() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.txt");
        let src_file = File::create(&src_path).unwrap();
        let src = src_path.to_str().unwrap();

        assert!(create_symlink(src, src)
            .unwrap_err()
            .contains("Source and destination directories are the same"));

        drop(src_file);
        dir.close().unwrap();
    }

    #[test]
    fn it_fails_when_src_dir_is_empty() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(create_symlink(src, dest)
            .unwrap_err()
            .contains("Source directory is empty"));

        src_dir.close().unwrap();
        dest_dir.close().unwrap();
    }

    // FIXME: this test also fails but the method is functioning correctly
    #[test]
    fn it_symlinks_files() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        let src_file = File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(create_symlink(src, dest).is_ok());

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.exists());

        src_dir.close().unwrap();
        drop(src_file);

        dest_dir.close().unwrap();
    }
}
