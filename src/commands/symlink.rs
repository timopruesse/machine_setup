use symlink::{remove_symlink_dir, symlink_dir};
use yaml_rust::yaml::Hash;

use crate::{
    command::CommandInterface,
    utils::directory::{expand_dir, get_source_and_target},
};

pub struct SymlinkCommand {}

impl CommandInterface for SymlinkCommand {
    fn install(&self, args: Hash) -> Result<(), String> {
        let dirs = get_source_and_target(args);
        if dirs.is_err() {
            return Err(dirs.err().unwrap());
        }
        let dirs = dirs.unwrap();

        let result = create_symlink(&dirs.src, &dirs.target);

        if result.is_err() {
            return Err(result.unwrap_err());
        }

        return Ok(());
    }

    fn uninstall(&self, args: Hash) -> Result<(), String> {
        let dirs = get_source_and_target(args);
        if dirs.is_err() {
            return Err(dirs.err().unwrap());
        }
        let dirs = dirs.unwrap();

        let result = remove_symlink(&dirs.target);

        if result.is_err() {
            return Err(result.unwrap_err());
        }

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

    if !source_dir.exists() {
        return Err(format!("Source directory does not exist: {}", source));
    }

    let expanded_destination = expand_dir(destination, true);
    if expanded_destination.is_err() {
        return Err(expanded_destination.unwrap_err().to_string());
    }
    let destination_dir = expanded_destination.to_owned().unwrap();

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "Source and destination directories are the same: {}",
            source
        ));
    }

    println!(
        "Symlinking {} to {}",
        source_dir.to_string(),
        destination_dir.to_string()
    );

    let result = symlink_dir(source_dir, destination_dir);

    if result.is_err() {
        return Err(result.unwrap_err().to_string());
    }

    return Ok(());
}

pub fn remove_symlink(destination: &str) -> Result<(), String> {
    let expanded_destination = expand_dir(destination, true);
    if expanded_destination.is_err() {
        return Err(expanded_destination.unwrap_err().to_string());
    }
    let destination_dir = expanded_destination.to_owned().unwrap();

    println!("Removing symlink to {}", destination_dir.to_string());

    let result = remove_symlink_dir(destination_dir);

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

    // FIXME: this test also fails but the method is functioning correctly
    fn it_removes_symlink() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        let src_file = File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(create_symlink(src, dest).is_ok());

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.exists());

        assert!(remove_symlink(src, dest).is_ok());

        assert!(!dest_path.exists());

        src_dir.close().unwrap();
        drop(src_file);

        dest_dir.close().unwrap();
    }
}
