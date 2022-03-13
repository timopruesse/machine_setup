use std::fs;
use std::path::Path;

pub fn copy_dir(source: &str, destination: &str) -> Result<(), String> {
    if !Path::new(source).exists() {
        return Err(format!("Source directory does not exist: {}", source));
    }

    if !Path::new(destination).exists() {
        fs::create_dir_all(destination).map_err(|e| format!("Failed to create destination directory: {}", e))?;
    }

    if source == destination {
        return Err(format!("Source and destination directories are the same: {}", source));
    }

    let mut source_files = fs::read_dir(source).map_err(|e| format!("Failed to read source directory: {}", e))?;
    if source_files.next().is_none() {
        return Err(format!("Source directory is empty: {}", source));
    }

    let mut destination_files = fs::read_dir(destination).map_err(|e| format!("Failed to read destination directory: {}", e))?;
    if destination_files.next().is_some() {
        return Err(format!("Destination directory is not empty: {}", destination));
    }

    for source_file in source_files {
        let source_file = source_file.map_err(|e| format!("Failed to read source file: {}", e))?;
        let source_path = source_file.path();
        let destination_path = destination.to_owned() + &source_path.to_str().unwrap().split(source).last().unwrap();
        fs::copy(source_path, destination_path).map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    Ok(())
}

// -- tests --

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn it_fails_when_src_dir_doesnt_exist() {
        assert!(copy_dir("invalid", "invalid").unwrap_err().contains("Source directory does not exist"));
    }

    #[test]
    fn it_fails_when_dirs_are_the_same() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("example.txt");
        let src_file = File::create(&src_path).unwrap();
        let src = src_path.to_str().unwrap();

        assert!(copy_dir(src, src).unwrap_err().contains("Source and destination directories are the same"));

        drop(src_file);
        dir.close().unwrap();
    }

    #[test]
    fn it_fails_when_src_dir_is_empty()
    {
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().to_str().unwrap();

        let dest_dir = tempdir().unwrap();
        let dest = dest_dir.path().to_str().unwrap();

        assert!(copy_dir(src, dest).unwrap_err().contains("Source directory is empty"));

        src_dir.close().unwrap();
        dest_dir.close().unwrap();
    }
}

