use ergo_fs::{expand, PathDir};
use std::fs;

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
