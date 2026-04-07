use std::path::{Path, PathBuf};

/// Expand `~` to the user's home directory and resolve relative paths.
pub fn expand_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let expanded = if let Some(stripped) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            let rest = stripped.strip_prefix('/').unwrap_or(stripped);
            home.join(rest)
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    // If path is relative and we have a base directory, resolve against it
    if expanded.is_relative() {
        if let Some(base) = base_dir {
            return base.join(&expanded);
        }
    }

    expanded
}

/// Check if a path should be ignored based on the ignore list.
pub fn should_ignore(path: &Path, ignore_list: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    ignore_list.iter().any(|pattern| {
        // Simple string matching — check if the file/dir name matches
        path.file_name()
            .is_some_and(|name| name.to_string_lossy() == *pattern)
            || path_str.contains(pattern.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_path("~/test", None);
        assert!(expanded.to_string_lossy().contains("test"));
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_relative_with_base() {
        let base = Path::new("/home/user/configs");
        let expanded = expand_path("./files", Some(base));
        assert_eq!(expanded, PathBuf::from("/home/user/configs/./files"));
    }

    #[test]
    fn test_expand_absolute_ignores_base() {
        let base = Path::new("/home/user/configs");
        let expanded = expand_path("/etc/config", Some(base));
        assert_eq!(expanded, PathBuf::from("/etc/config"));
    }

    #[test]
    fn test_should_ignore() {
        assert!(should_ignore(
            Path::new("/path/to/README.md"),
            &["README.md".to_string()]
        ));
        assert!(!should_ignore(
            Path::new("/path/to/config.yaml"),
            &["README.md".to_string()]
        ));
    }
}
