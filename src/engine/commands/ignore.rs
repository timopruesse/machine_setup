//! Ignore matching for Tree materialization walks.
//!
//! Pattern language (no external glob crate):
//! - **Basename / component exact match** — `node_modules` matches any path
//!   component named exactly `node_modules` (not a substring).
//! - **Relative path sequence** — `foo/bar` matches when consecutive components
//!   equal that sequence anywhere in the relative path (suffix or infix).
//! - **Glob-lite** — `*` and `?` apply within a single path component only;
//!   e.g. `*.o` matches `foo.o` but not `foo/bar.o`'s parent path as one unit.

use std::path::{Component, Path};

/// Returns true when `relative` (path relative to the walk root) matches any
/// ignore pattern in `patterns`.
pub fn should_ignore(relative: &Path, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_pattern(relative, pattern))
}

fn matches_pattern(relative: &Path, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    if pattern.contains('/') {
        return matches_path_sequence(relative, pattern);
    }

    if pattern.contains('*') || pattern.contains('?') {
        return relative.components().any(|component| match component {
            Component::Normal(name) => glob_matches(&name.to_string_lossy(), pattern),
            _ => false,
        });
    }

    relative.components().any(|component| match component {
        Component::Normal(name) => name == pattern,
        _ => false,
    })
}

fn path_components(relative: &Path) -> Vec<String> {
    relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

fn matches_path_sequence(relative: &Path, pattern: &str) -> bool {
    let pattern_components: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if pattern_components.is_empty() {
        return false;
    }

    let path_components = path_components(relative);
    if path_components.len() < pattern_components.len() {
        return false;
    }

    path_components
        .windows(pattern_components.len())
        .any(|window| {
            window
                .iter()
                .zip(pattern_components.iter())
                .all(|(path_seg, pat_seg)| component_matches(path_seg, pat_seg))
        })
}

fn component_matches(path_segment: &str, pattern_segment: &str) -> bool {
    if pattern_segment.contains('*') || pattern_segment.contains('?') {
        glob_matches(path_segment, pattern_segment)
    } else {
        path_segment == pattern_segment
    }
}

/// Glob-lite: `*` matches zero or more chars, `?` matches exactly one char,
/// both confined to a single path component (pattern must not contain `/`).
fn glob_matches(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    glob_matches_at(&text_chars, &pattern_chars, 0, 0)
}

fn glob_matches_at(text: &[char], pattern: &[char], text_idx: usize, pat_idx: usize) -> bool {
    if pat_idx == pattern.len() {
        return text_idx == text.len();
    }

    if pattern[pat_idx] == '*' {
        for i in text_idx..=text.len() {
            if glob_matches_at(text, pattern, i, pat_idx + 1) {
                return true;
            }
        }
        return false;
    }

    if text_idx >= text.len() {
        return false;
    }

    if pattern[pat_idx] == '?' || pattern[pat_idx] == text[text_idx] {
        return glob_matches_at(text, pattern, text_idx + 1, pat_idx + 1);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_component_matches_basename_only_not_substring() {
        assert!(should_ignore(
            Path::new("path/to/README.md"),
            &["README.md".to_string()]
        ));
        assert!(!should_ignore(
            Path::new("path/to/config.yaml"),
            &["README.md".to_string()]
        ));
    }

    #[test]
    fn tmp_does_not_match_template() {
        assert!(!should_ignore(
            Path::new("src/template.rs"),
            &["tmp".to_string()]
        ));
        assert!(!should_ignore(Path::new("template"), &["tmp".to_string()]));
        assert!(should_ignore(
            Path::new("build/tmp/output"),
            &["tmp".to_string()]
        ));
    }

    #[test]
    fn glob_o_matches_object_files() {
        assert!(should_ignore(Path::new("foo.o"), &["*.o".to_string()]));
        assert!(should_ignore(
            Path::new("build/foo.o"),
            &["*.o".to_string()]
        ));
        assert!(!should_ignore(Path::new("foo.obj"), &["*.o".to_string()]));
    }

    #[test]
    fn node_modules_matches_any_component() {
        assert!(should_ignore(
            Path::new("packages/app/node_modules/pkg/index.js"),
            &["node_modules".to_string()]
        ));
        assert!(!should_ignore(
            Path::new("node_modules_backup/file"),
            &["node_modules".to_string()]
        ));
    }

    #[test]
    fn path_sequence_matches_infix_and_suffix() {
        assert!(should_ignore(
            Path::new("a/foo/bar/b"),
            &["foo/bar".to_string()]
        ));
        assert!(should_ignore(
            Path::new("prefix/foo/bar"),
            &["foo/bar".to_string()]
        ));
        assert!(!should_ignore(
            Path::new("foo/baz/bar"),
            &["foo/bar".to_string()]
        ));
    }
}
