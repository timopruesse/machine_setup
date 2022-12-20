use ansi_term::Color::{Green, Red, White, Yellow};
use ergo_fs::{Path, PathArc};
use indicatif::ProgressBar;
use std::fs::remove_file;
use symlink::{remove_symlink_file, symlink_file};
use tracing::debug;

use crate::{
    command::{CommandConfig, CommandInterface},
    config::config_value::ConfigValue,
    utils::directory::{expand_path, get_source_and_target, walk_files},
};

pub struct SymlinkCommand {}

fn should_force(args: ConfigValue) -> bool {
    if !args.is_hash() {
        return false;
    }

    let arg_values = args.as_hash().unwrap();

    if let Some(force) = arg_values.get("force") {
        return force.as_bool().unwrap_or(false);
    }

    false
}

impl CommandInterface for SymlinkCommand {
    fn install(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let dirs = get_source_and_target(args.clone(), &config.config_dir)?;

        create_symlink(
            &dirs.src,
            &dirs.target,
            dirs.ignore,
            should_force(args),
            progress,
        )
    }

    fn uninstall(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        let dirs = get_source_and_target(args, &config.config_dir)?;

        remove_symlink(&dirs.src, &dirs.target, progress)
    }

    fn update(
        &self,
        args: ConfigValue,
        config: &CommandConfig,
        progress: &ProgressBar,
    ) -> Result<(), String> {
        self.install(args, config, progress)
    }
}

fn link_files(
    source_dir: &PathArc,
    destination_dir: &Path,
    ignore: Vec<ConfigValue>,
    force: bool,
    progress: &ProgressBar,
) -> Result<(), String> {
    let message = format!(
        "Creating symlinks: {} {} {} ...",
        White.bold().paint(source_dir.to_string()),
        Green.bold().paint("->"),
        White.bold().paint(destination_dir.to_str().unwrap())
    );

    debug!(message);
    progress.set_message(message);

    walk_files(source_dir, destination_dir, ignore, |src, target| {
        debug!(
            "Linking {} to {} ...",
            White.bold().paint(src.to_str().unwrap()),
            White.bold().paint(target.to_str().unwrap())
        );

        if force && target.is_file() {
            debug!(
                "{}",
                Yellow.paint("Replacing exisiting file with symlink (force) ...")
            );

            remove_file(target).ok();
        }

        symlink_file(src, target)
            .map_err(|e| format!("Failed to link file: {}", Red.paint(e.to_string())))
            .ok();
    })
}

fn unlink_files(
    source_dir: &PathArc,
    destination_dir: &Path,
    progress: &ProgressBar,
) -> Result<(), String> {
    let message = format!(
        "Unlinking files in {} ...",
        White.bold().paint(destination_dir.to_str().unwrap())
    );

    debug!(message);
    progress.set_message(message);

    walk_files(source_dir, destination_dir, vec![], |_src, target| {
        debug!(
            "Unlinking {} ...",
            White.bold().paint(target.to_str().unwrap())
        );
        remove_symlink_file(target)
            .map_err(|e| format!("Failed to unlink file: {}", Red.paint(e.to_string())))
            .ok();
    })
}

pub fn create_symlink(
    source: &str,
    destination: &str,
    ignore: Vec<ConfigValue>,
    force: bool,
    progress: &ProgressBar,
) -> Result<(), String> {
    let source_dir = expand_path(source, false)?;

    if !source_dir.exists() {
        return Err(format!("Source directory does not exist: {}", source));
    }

    let destination_dir = expand_path(destination, true)?;

    if source_dir.to_string() == destination_dir.to_string() {
        return Err(format!(
            "Source and destination directories are the same: {}",
            source
        ));
    }

    link_files(&source_dir, &destination_dir, ignore, force, progress)
}

pub fn remove_symlink(
    source: &str,
    destination: &str,
    progress: &ProgressBar,
) -> Result<(), String> {
    let source_dir = expand_path(source, false)?;
    let destination_dir = expand_path(destination, false)?;

    unlink_files(&source_dir, &destination_dir, progress)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{fs::File, vec};
    use tempfile::tempdir;

    #[test]
    fn it_fails_when_dirs_are_the_same() {
        let dir = tempdir().unwrap();
        let src_path = dir.path();
        File::create(&src_path.join("example.txt")).unwrap();

        let src = src_path.to_str().unwrap();

        let pb = ProgressBar::new(0);

        assert!(create_symlink(src, src, vec![], false, &pb)
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

        let pb = ProgressBar::new(0);

        create_symlink(src, dest, vec![], false, &pb).unwrap();

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.is_symlink())
    }

    #[test]
    fn it_overrides_file_with_symlink() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();
        let dest_path = dest_dir.path().join("example.txt");

        File::create(&dest_path).unwrap();

        let pb = ProgressBar::new(0);

        create_symlink(src, dest, vec![], true, &pb).unwrap();

        assert!(dest_path.is_symlink());
    }

    #[test]
    fn it_removes_symlink() {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();
        let src_path = src_dir.path().join("example.txt");
        File::create(&src_path).unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        let pb = ProgressBar::new(0);

        create_symlink(src, dest, vec![], false, &pb).unwrap();

        let dest_path = dest_dir.path().join("example.txt");
        assert!(dest_path.exists());

        remove_symlink(src, dest, &pb).unwrap();

        assert!(!dest_path.exists());
    }
}
