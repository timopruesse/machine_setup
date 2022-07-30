use ergo_fs::{expand, Path, PathArc, PathDir, WalkDir};
use std::{collections::HashMap, fs::create_dir_all};
use tracing::info;

use crate::config::{
    config_value::ConfigValue, validation_rules::required::Required, validator::validate_named_args,
};

pub fn is_file_path(path: &PathArc) -> bool {
    if path.to_str().unwrap().is_empty() {
        return false;
    }

    let last_component = path
        .components()
        .last()
        .unwrap()
        .as_os_str()
        .to_owned()
        .into_string()
        .unwrap();

    let dot_index = last_component.find('.').unwrap_or(0);

    dot_index != 0
}

fn create_missing_directories(path: &PathArc) -> Result<(), std::io::Error> {
    if is_file_path(path) {
        let parent = path.parent();

        if parent.is_none() {
            return Ok(());
        }
        return create_dir_all(parent.unwrap());
    }

    create_dir_all(path)
}

pub fn expand_path(path: &str, create: bool) -> Result<PathArc, String> {
    let expanded_path = expand(path);
    if let Err(err_expand_path) = expanded_path {
        return Err(err_expand_path.to_string());
    }
    let expanded_path = PathArc::new(expanded_path.unwrap().to_string());

    if create {
        let create_result = create_missing_directories(&expanded_path);
        if let Err(err_create_missing_directories) = create_result {
            return Err(err_create_missing_directories.to_string());
        }
    }

    Ok(expanded_path)
}

pub static DIR_SRC: &str = "src";
pub static DIR_TARGET: &str = "target";
pub static DIR_IGNORE: &str = "ignore";

pub struct Dirs {
    pub src: String,
    pub target: String,
    pub ignore: Vec<ConfigValue>,
}

pub fn get_relative_dir(root: &PathDir, dir: &str) -> String {
    if dir.starts_with('~') {
        return dir.to_string();
    }

    root.join(dir).to_string()
}

pub fn get_source_and_target(args: ConfigValue, root: &PathDir) -> Result<Dirs, String> {
    let rules = vec![&Required {}];

    validate_named_args(
        args.to_owned(),
        HashMap::from([
            (String::from(DIR_SRC), rules.clone()),
            (String::from(DIR_TARGET), rules.clone()),
        ]),
    )?;

    let src_dir = args
        .as_hash()
        .unwrap()
        .get(DIR_SRC)
        .unwrap()
        .as_str()
        .unwrap();

    let target_dir = args
        .as_hash()
        .unwrap()
        .get(DIR_TARGET)
        .unwrap()
        .as_str()
        .unwrap();

    let relative_target_dir = get_relative_dir(root, target_dir);
    if relative_target_dir.is_empty() {
        return Err(String::from("Target directory cannot be empty"));
    }

    let relative_src_dir = get_relative_dir(root, src_dir);

    let ignore = args
        .as_hash()
        .unwrap()
        .get(DIR_IGNORE)
        .unwrap_or(&ConfigValue::Array(vec![]))
        .as_vec()
        .unwrap()
        .to_owned();

    Ok(Dirs {
        src: relative_src_dir,
        target: relative_target_dir,
        ignore,
    })
}

// TODO: improve this with a better regex approach :)
fn is_ignored(path: &Path, source: &PathArc, ignore: &[ConfigValue]) -> bool {
    let path_str = path.strip_prefix(source).unwrap().to_str().unwrap();

    for ignore_path in ignore {
        let ignore_path = ignore_path.as_str().unwrap();
        if path_str.starts_with(ignore_path) {
            return true;
        }
    }

    false
}

pub fn walk_files<O: Fn(&Path, &Path)>(
    source: &PathArc,
    target: &Path,
    ignore: Vec<ConfigValue>,
    op: O,
) -> Result<(), String> {
    if !source.exists() {
        return Err(format!(
            "Source directory/file does not exist: {}",
            source.to_string_lossy()
        ));
    }

    if source.is_file() {
        let source_file_name = source.file_name().unwrap().to_str().unwrap();
        let source_file_ending = source_file_name.split('.').last().unwrap();

        let target_file_ending = target
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split('.')
            .last()
            .unwrap();

        if source_file_ending != target_file_ending {
            op(source, target.join(source_file_name).as_path())
        } else {
            op(source, target)
        }

        return Ok(());
    }

    for dir_entry in WalkDir::new(&source).min_depth(1) {
        let dir_entry = dir_entry.unwrap();
        let source_path = dir_entry.path();

        if is_ignored(source_path, source, &ignore) {
            info!("Skipping {} ...", source_path.to_string_lossy());
            continue;
        }

        let destination_path = target.join(source_path.strip_prefix(&source).unwrap());

        if source_path.is_dir() {
            let create_result = create_dir_all(&destination_path);
            if let Err(err_create) = create_result {
                return Err(err_create.to_string());
            }
            continue;
        }

        op(source_path, &destination_path);
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn it_fails_when_dir_doesnt_exist() {
        let source = PathArc::new("/tmp/does_not_exist");
        let target = PathArc::new("/tmp/target");

        walk_files(&source, &target, vec![], |_, _| {}).unwrap_err();
    }

    #[test]
    fn it_expands_str_to_path() {
        let expanded_dir = expand_path("~", false);

        assert!(expanded_dir.is_ok());
        assert_eq!(
            expanded_dir.unwrap().to_string_lossy(),
            dirs::home_dir().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn it_returns_true_if_the_path_is_a_file() {
        assert!(is_file_path(&PathArc::new("/tmp/test.txt")));
    }

    #[test]
    fn it_returns_false_if_the_path_is_a_directory() {
        assert!(!is_file_path(&PathArc::new("/tmp/test")));
    }

    #[test]
    fn it_creates_intermediate_dirs_when_needed() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path = temp_dir.path();
        let target_dir = temp_dir_path.join("test");

        let expanded_dir = expand_path(target_dir.to_str().unwrap(), true);

        assert!(expanded_dir.is_ok());
        assert_eq!(
            expanded_dir.unwrap().to_string_lossy(),
            target_dir.to_string_lossy()
        );
        assert!(target_dir.exists());
    }
}
