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
    pub path: String,
    pub file: File,
}

pub fn create_temp_file(file_ending: &str, temp_dir: &str) -> Result<FileInfo, String> {
    let expanded_temp_dir = expand_path(temp_dir, true);
    if let Err(err_expand) = expanded_temp_dir {
        return Err(err_expand);
    }

    let expanded_temp_dir = expanded_temp_dir.unwrap().to_string();

    let file_name = format!("{}.{}", get_random_string(), file_ending);
    let file_path = format!("{}/{}", expanded_temp_dir, file_name);

    let file_result = File::create(&file_path);
    if let Err(err_file) = file_result {
        return Err(err_file.to_string());
    }
    let file = file_result.unwrap();

    Ok(FileInfo {
        path: file_path,
        file,
    })
}
