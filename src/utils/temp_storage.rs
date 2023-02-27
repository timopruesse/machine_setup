use ergo_fs::PathBuf;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::fs::File;

use super::directory::expand_path;

fn get_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(25)
        .map(char::from)
        .collect()
}

pub struct FileInfo {
    pub path: PathBuf,
    pub file: File,
}

pub fn create_temp_file(file_ending: &str, temp_dir: &str) -> Result<FileInfo, String> {
    let expanded_dir = expand_path(temp_dir, true)?;
    let expanded_temp_dir = expanded_dir.to_str().unwrap();

    let file_name = format!("{}.{}", get_random_string(), file_ending);
    let file_path = format!("{}/{}", expanded_temp_dir, file_name);

    let file = match File::create(&file_path) {
        Ok(file) => file,
        Err(error) => return Err(error.to_string()),
    };

    Ok(FileInfo {
        path: PathBuf::from(&file_path),
        file,
    })
}
