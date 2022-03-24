use ergo_fs::{expand, Path, PathDir, WalkDir};
use std::fs;
use yaml_rust::{yaml::Hash, Yaml};

use crate::command::validate_args;

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

pub fn get_source_and_target(args: Hash) -> Result<Dirs, String> {
    let validation = validate_args(
        args.to_owned(),
        vec![String::from(DIR_SRC), String::from(DIR_TARGET)],
    );
    if validation.is_err() {
        return Err(validation.unwrap_err());
    }

    let src_dir = args
        .get(&Yaml::String(String::from(DIR_SRC)))
        .unwrap()
        .as_str()
        .unwrap();

    let target_dir = args
        .get(&Yaml::String(String::from(DIR_TARGET)))
        .unwrap()
        .as_str()
        .unwrap();

    if target_dir.is_empty() {
        return Err(String::from("Target directory cannot be empty"));
    }

    let ignore = args
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
