use ergo_fs::{expand, Path, PathDir, WalkDir};
use std::{collections::HashMap, fs};
use yaml_rust::Yaml;

use crate::config::{validation_rules::required::Required, validator::validate_named_args};

pub fn expand_dir(dir: &str, create: bool) -> Result<PathDir, String> {
    let expanded_dir = expand(dir);

    if expanded_dir.is_err() {
        return Err(expanded_dir.unwrap_err().to_string());
    }

    let expanded_dir = expanded_dir.unwrap();
    if create {
        let create_result = fs::create_dir_all(expanded_dir.to_string());
        if create_result.is_err() {
            return Err(create_result.unwrap_err().to_string());
        }
    }

    let path = PathDir::new(expanded_dir.to_string());

    if path.is_err() {
        return Err(path.unwrap_err().to_string());
    }

    return Ok(path.unwrap());
}

pub static DIR_SRC: &str = "src";
pub static DIR_TARGET: &str = "target";
pub static DIR_IGNORE: &str = "ignore";

pub struct Dirs {
    pub src: String,
    pub target: String,
    pub ignore: Vec<Yaml>,
}

pub fn get_source_and_target(args: Yaml) -> Result<Dirs, String> {
    let rules = vec![&Required {}];

    let validation = validate_named_args(
        args.to_owned(),
        HashMap::from([
            (String::from(DIR_SRC), rules.clone()),
            (String::from(DIR_TARGET), rules.clone()),
        ]),
    );
    if validation.is_err() {
        return Err(validation.unwrap_err());
    }

    let src_dir = args
        .as_hash()
        .unwrap()
        .get(&Yaml::String(String::from(DIR_SRC)))
        .unwrap()
        .as_str()
        .unwrap();

    let target_dir = args
        .as_hash()
        .unwrap()
        .get(&Yaml::String(String::from(DIR_TARGET)))
        .unwrap()
        .as_str()
        .unwrap();

    if target_dir.is_empty() {
        return Err(String::from("Target directory cannot be empty"));
    }

    let ignore = args
        .as_hash()
        .unwrap()
        .get(&Yaml::String(String::from(DIR_IGNORE)))
        .unwrap_or(&Yaml::Array(vec![]))
        .as_vec()
        .unwrap()
        .to_owned();

    return Ok(Dirs {
        src: src_dir.to_string(),
        target: target_dir.to_string(),
        ignore,
    });
}

static DEFAULT_IGNORE: [&str; 3] = [".git", ".gitignore", ".gitmodules"];

fn is_ignored(path: &Path, source: &PathDir, ignore: &Vec<Yaml>) -> bool {
    let path_str = path.strip_prefix(source).unwrap().to_str().unwrap();

    let mut ignore_list = ignore.to_owned();
    ignore_list.extend_from_slice(&DEFAULT_IGNORE.map(|s| Yaml::String(s.to_string())));

    for ignore_path in ignore_list {
        let ignore_path = ignore_path.as_str().unwrap();
        if path_str.starts_with(ignore_path) {
            return true;
        }
    }

    return false;
}

pub fn walk_files<O: Fn(&Path, &Path)>(
    source: &PathDir,
    target: &Path,
    ignore: Vec<Yaml>,
    op: O,
) -> Result<(), String> {
    for dir_entry in WalkDir::new(&source).min_depth(1) {
        let dir_entry = dir_entry.unwrap();
        let source_path = dir_entry.path();

        if is_ignored(source_path, &source, &ignore) {
            println!("Skipping {} ...", source_path.to_string_lossy());
            continue;
        }

        let destination_path = target.join(source_path.strip_prefix(&source).unwrap());

        if source_path.is_dir() {
            let create_result = fs::create_dir_all(&destination_path);
            if create_result.is_err() {
                return Err(create_result.unwrap_err().to_string());
            }
            continue;
        }

        op(&source_path, &destination_path);
    }

    return Ok(());
}

// -- tests --

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_expands_str_to_path() {
        let expanded_dir = expand_dir("~/test", false);

        assert!(expanded_dir.is_ok());
        assert_eq!(
            expanded_dir.unwrap().to_string_lossy(),
            dirs::home_dir().unwrap().join("test").to_string_lossy()
        );
    }

    #[test]
    fn it_creates_intermediate_dirs_when_needed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();
        let temp_dir_path_complete = temp_dir_path.join("test");

        let expanded_dir = expand_dir(temp_dir_path_complete.to_str().unwrap(), true);

        assert!(expanded_dir.is_ok());
        assert_eq!(
            expanded_dir.unwrap().to_string_lossy(),
            temp_dir_path_complete.to_string_lossy()
        );
        assert!(temp_dir_path_complete.exists());
    }
}
