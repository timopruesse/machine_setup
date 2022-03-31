use ergo_fs::{Path, PathDir};
use symlink::{remove_symlink_file, symlink_file};
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    utils::directory::{expand_dir, get_source_and_target, walk_files},
};

pub struct SymlinkCommand {}

impl CommandInterface for SymlinkCommand {
    fn install(&self, args: Yaml) -> Result<(), String> {
        let dirs = get_source_and_target(args);
        if let Err(err_dirs) = dirs {
            return Err(err_dirs);
        }
        let dirs = dirs.unwrap();

        let result = create_symlink(&dirs.src, &dirs.target, dirs.ignore);

        if let Err(err_result) = result {
            return Err(err_result);
        }

        Ok(())
    }

    fn uninstall(&self, args: Yaml) -> Result<(), String> {
        let dirs = get_source_and_target(args);
        if dirs.is_err() {
            return Err(dirs.err().unwrap());
        }
        let dirs = dirs.unwrap();

        let result = remove_symlink(&dirs.src, &dirs.target);

        if let Err(err_result) = result {
            return Err(err_result);
        }

        Ok(())
    }

    fn update(&self, _args: Yaml) -> Result<(), String> {
        unimplemented!()
    }
}

fn link_files(
    source_dir: &PathDir,
    destination_dir: &Path,
    ignore: Vec<Yaml>,
) -> Result<(), String> {
    println!(
        "Creating symlinks: {} -> {} ...",
        source_dir.to_string(),
        destination_dir.to_str().unwrap()
    );

    let result = walk_files(source_dir, destination_dir, ignore, |src, target| {
        println!(
            "Linking {} to {} ...",
            src.to_str().unwrap(),
            target.to_str().unwrap()
        );
        symlink_file(src, target)
            .map_err(|e| format!("Failed to link file: {}", e))
            .ok();
    });

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(())
}

fn unlink_files(source_dir: &PathDir, destination_dir: &Path) -> Result<(), String> {
    println!(
        "Unlinking files in {} ...",
        destination_dir.to_str().unwrap()
    );

    let result = walk_files(source_dir, destination_dir, vec![], |_src, target| {
        println!("Unlinking {} ...", target.to_str().unwrap());
        remove_symlink_file(target)
            .map_err(|e| format!("Failed to unlink file: {}", e))
            .ok();
    });

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(())
}

pub fn create_symlink(source: &str, destination: &str, ignore: Vec<Yaml>) -> Result<(), String> {
    let expanded_source = expand_dir(source, false);
    if let Err(err_expand_src) = expanded_source {
        return Err(err_expand_src);
    }
    let source_dir = expanded_source.to_owned().unwrap();

    if !source_dir.exists() {
        return Err(format!("Source directory does not exist: {}", source));
    }

    let expanded_destination = expand_dir(destination, true);
    if let Err(err_expand_dest) = expanded_destination {
        return Err(err_expand_dest);
    }
    let destination_dir = expanded_destination.unwrap();

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "Source and destination directories are the same: {}",
            source
        ));
    }

    let result = link_files(&source_dir, &destination_dir, ignore);

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(())
}

pub fn remove_symlink(source: &str, destination: &str) -> Result<(), String> {
    let expanded_source = expand_dir(source, false);
    if let Err(err_expand_src) = expanded_source {
        return Err(err_expand_src);
    }
    let source_dir = expanded_source.to_owned().unwrap();

    let expanded_destination = expand_dir(destination, false);
    if let Err(err_expand_dest) = expanded_destination {
        return Err(err_expand_dest);
    }
    let destination_dir = expanded_destination.unwrap();

    println!("Removing symlink to {} ...", destination_dir.to_string());

    let result = unlink_files(&source_dir, &destination_dir);

    if let Err(err_result) = result {
        return Err(err_result);
    }

    Ok(())
}

// -- tests --

#[cfg(test)]
mod test {
    use super::*;
    use std::{fs::File, vec};
    use tempfile::tempdir;

    #[test]
    fn it_fails_when_src_dir_doesnt_exist() {
        assert!(create_symlink("invalid", "invalid", vec![])
            .unwrap_err()
            .contains("path is not a dir when resolving"));
    }

    #[test]
    fn it_fails_when_dirs_are_the_same() {
        let dir = tempdir().unwrap();
        let src_path = dir.path();
        File::create(&src_path.join("example.txt")).unwrap();

        let src = src_path.to_str().unwrap();

        println!("{:?}", create_symlink(src, src, vec![]));

        assert!(create_symlink(src, src, vec![])
            .unwrap_err()
            .contains("Source and destination directories are the same"));
    }

    #[test]
    fn it_symlinks_files() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(create_symlink(src, dest, vec![]).is_ok());

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.exists());
    }

    #[test]
    fn it_removes_symlink() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(create_symlink(src, dest, vec![]).is_ok());

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.exists());

        assert!(remove_symlink(src, dest).is_ok());

        assert!(!dest_path.exists());
    }
}
