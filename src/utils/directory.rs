use ergo_fs::{expand, Path, PathBuf, PathDir, WalkDir};
use std::{
    collections::{HashMap, HashSet},
    fs::create_dir_all,
};
use tracing::info;

use crate::config::{
    config_value::ConfigValue,
    validation_rules::required::Required,
    validator::{validate_named_args, ValidationRule},
};

pub fn is_file_path(path: &Path) -> bool {
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

fn create_missing_directories(path: &PathBuf) -> Result<(), std::io::Error> {
    if is_file_path(path) {
        let parent = path.parent();

        if parent.is_none() {
            return Ok(());
        }
        return create_dir_all(parent.unwrap());
    }

    create_dir_all(path)
}

pub fn expand_path(path: &str, create: bool) -> Result<PathBuf, String> {
    let expanded_path = expand(path).map_err(|err| err.to_string())?;
    let expanded_path = PathBuf::from(expanded_path.to_string());

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
    pub ignore: HashSet<String>,
}

pub fn get_relative_dir(root: &PathDir, dir: &str) -> String {
    if dir.starts_with('~') {
        return dir.to_string();
    }

    root.join(dir).to_string()
}

pub fn get_source_and_target(args: ConfigValue, root: &PathDir) -> Result<Dirs, String> {
    let src_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];
    let target_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];

    validate_named_args(
        args.to_owned(),
        HashMap::from([
            (String::from(DIR_SRC), src_rules),
            (String::from(DIR_TARGET), target_rules),
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

    let ignore: HashSet<String> = args
        .as_hash()
        .unwrap()
        .get(DIR_IGNORE)
        .unwrap_or(&ConfigValue::Array(vec![]))
        .as_vec()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();

    Ok(Dirs {
        src: relative_src_dir,
        target: relative_target_dir,
        ignore,
    })
}

// TODO: improve this with a better regex approach :)
fn is_ignored(path: &Path, source: &Path, ignore: &HashSet<String>) -> bool {
    if let Ok(rel_path) = path.strip_prefix(source) {
        if let Some(rel_str) = rel_path.to_str() {
            return ignore.contains(rel_str);
        }
    }
    false
}

pub fn walk_files<O: Fn(&Path, &Path)>(
    source: &PathBuf,
    target: &Path,
    ignore: HashSet<String>,
    op: O,
) -> Result<(), String> {
    if !source.exists() {
        return Err(format!(
            "Source directory/file does not exist: {}",
            source.to_string_lossy()
        ));
    }

    if source.is_file() {
        let source_ext = source.extension().unwrap_or_default();
        let target_ext = target.extension().unwrap_or_default();

        match source_ext == target_ext {
            true => {
                op(source, target);
            }
            false => {
                op(source, target.join(source.file_name().unwrap()).as_path());
            }
        }

        return Ok(());
    }

    for dir_entry in WalkDir::new(source).min_depth(1).into_iter() {
        let dir_entry = dir_entry.unwrap();
        let source_path = dir_entry.path();

        if is_ignored(source_path, source, &ignore) {
            info!("Skipping {} ...", source_path.to_string_lossy());
            continue;
        }

        let destination_path = target.join(source_path.strip_prefix(source).unwrap());

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
        let source = PathBuf::from("/tmp/does_not_exist");
        let target = PathBuf::from("/tmp/target");

        walk_files(&source, &target, HashSet::new(), |_, _| {}).unwrap_err();
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
        assert!(is_file_path(&PathBuf::from("/tmp/test.txt")));
    }

    #[test]
    fn it_returns_false_if_the_path_is_a_directory() {
        assert!(!is_file_path(&PathBuf::from("/tmp/test")));
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
