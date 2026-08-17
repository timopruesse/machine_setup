use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::error::Result;

/// Expand `~` to the user's home directory, `$VAR` to environment variables,
/// and resolve relative paths against the base directory.
pub fn expand_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    // First, expand environment variables ($VAR or ${VAR})
    let path = expand_env_vars(path);

    let expanded = if let Some(stripped) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            let rest = stripped.strip_prefix('/').unwrap_or(stripped);
            home.join(rest)
        } else {
            PathBuf::from(path.as_str())
        }
    } else {
        PathBuf::from(path.as_str())
    };

    // If path is relative and we have a base directory, resolve against it
    if expanded.is_relative() {
        if let Some(base) = base_dir {
            return base.join(&expanded);
        }
    }

    expanded
}

/// Expand `$VAR` and `${VAR}` references in a string using process environment variables.
fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek() == Some(&'{') {
                // ${VAR} form
                chars.next(); // consume '{'
                let var_name: String = chars.by_ref().take_while(|&c| c != '}').collect();
                if let Ok(val) = std::env::var(&var_name) {
                    result.push_str(&val);
                } else {
                    // Keep original if not found
                    result.push('$');
                    result.push('{');
                    result.push_str(&var_name);
                    result.push('}');
                }
            } else {
                // $VAR form — var name is alphanumeric + underscore
                let mut var_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        var_name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if var_name.is_empty() {
                    result.push('$');
                } else if let Ok(val) = std::env::var(&var_name) {
                    result.push_str(&val);
                } else {
                    // Keep original if not found
                    result.push('$');
                    result.push_str(&var_name);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Walk `src` and invoke `f` for each entry that isn't filtered out by
/// `ignore_list`. The closure receives the raw `DirEntry` plus the
/// precomputed destination path (`target` joined with the entry's
/// `src`-relative suffix).
///
/// Ignored **directories** are not descended into; ignored **files** are
/// skipped. The walk root (`src`) is never pruned by ignore.
///
/// `strip_prefix` is guaranteed to succeed because every WalkDir entry is
/// rooted at `src`.
pub fn walk_relative<F>(src: &Path, target: &Path, ignore_list: &[String], mut f: F) -> Result<()>
where
    F: FnMut(&DirEntry, &Path) -> Result<()>,
{
    let walker = WalkDir::new(src).into_iter().filter_entry(|entry| {
        // Never prune the walk root — ignore applies only to descendants.
        if entry.depth() == 0 {
            return true;
        }
        let relative = entry.path().strip_prefix(src).unwrap_or(entry.path());
        !should_ignore(relative, ignore_list)
    });

    for entry in walker.filter_map(|e| e.ok()) {
        let relative = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let dest = target.join(relative);
        f(&entry, &dest)?;
    }
    Ok(())
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

/// Compact path for log output: replace home directory prefix with `~/`.
pub fn shorten_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            if stripped.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorten_path_uses_tilde_for_home() {
        if let Some(home) = dirs::home_dir() {
            let p = home.join("dotfiles/.zshrc");
            assert_eq!(shorten_path(&p), "~/dotfiles/.zshrc");
            assert_eq!(shorten_path(&home), "~");
        }
    }

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
    fn test_env_var_resolved() {
        std::env::set_var("TEST_MS_DIR", "/opt/dotfiles");
        let base = Path::new("/home/user/configs");
        let expanded = expand_path("$TEST_MS_DIR/personal_repositories.yaml", Some(base));
        assert_eq!(
            expanded,
            PathBuf::from("/opt/dotfiles/personal_repositories.yaml")
        );
        std::env::remove_var("TEST_MS_DIR");
    }

    #[test]
    fn test_env_var_braced_resolved() {
        std::env::set_var("TEST_MS_DIR2", "/opt/dotfiles");
        let expanded = expand_path("${TEST_MS_DIR2}/config.yaml", None);
        assert_eq!(expanded, PathBuf::from("/opt/dotfiles/config.yaml"));
        std::env::remove_var("TEST_MS_DIR2");
    }

    #[test]
    fn test_env_var_unknown_kept() {
        let expanded = expand_path("$NONEXISTENT_VAR_12345/file.yaml", None);
        // Unknown vars are kept as-is but still treated as a path
        assert!(expanded.to_string_lossy().contains("NONEXISTENT_VAR_12345"));
    }

    #[test]
    fn test_home_env_var() {
        let expanded = expand_path("$HOME/.config", None);
        assert!(!expanded.to_string_lossy().contains('$'));
        assert!(expanded.to_string_lossy().contains(".config"));
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

    #[test]
    fn walk_relative_prunes_ignored_directory_children() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("keep")).unwrap();
        std::fs::create_dir_all(src.join("skip_me/nested")).unwrap();
        std::fs::write(src.join("keep/a.txt"), b"a").unwrap();
        std::fs::write(src.join("skip_me/secret.txt"), b"s").unwrap();
        std::fs::write(src.join("skip_me/nested/deep.txt"), b"d").unwrap();
        std::fs::write(src.join("root.txt"), b"r").unwrap();

        let target = dir.path().join("dst");
        let mut seen = Vec::new();
        walk_relative(&src, &target, &["skip_me".to_string()], |entry, _dest| {
            let rel = entry
                .path()
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            seen.push(rel);
            Ok(())
        })
        .unwrap();

        seen.sort();
        assert!(
            !seen
                .iter()
                .any(|p| p.contains("skip_me") || p.contains("secret") || p.contains("deep")),
            "ignored subtree must not be visited: {seen:?}"
        );
        assert!(seen
            .iter()
            .any(|p| p == "root.txt" || p.ends_with("root.txt")));
        assert!(seen.iter().any(|p| p.contains("a.txt")));
    }
}
