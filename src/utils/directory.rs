use ergo_fs::{expand, PathDir};
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

static DIR_SRC: &str = "src";
static DIR_TARGET: &str = "target";

pub struct Dirs {
    pub src: String,
    pub target: String,
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

    return Ok(Dirs {
        src: src_dir.to_string(),
        target: target_dir.to_string(),
    });
}
