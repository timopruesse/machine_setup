//! Config document — create and append-only authoring of the user YAML file.
//!
//! Does not serde-round-trip existing files (comments would die). `init` writes
//! a fresh shell; `add_task` appends a Task stub. In-place rewrite is deferred.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

use super::locator;
use super::types::{blank_task_config, AppConfig, TaskConfig};
use super::{load_config, resolve_config_dir, validate};

/// Stable raw URL for the checked-in Config schema (yaml-language-server modeline).
pub const SCHEMA_MODELINE_URL: &str =
    "https://raw.githubusercontent.com/timopruesse/machine_setup/main/schema/machine_setup.schema.json";

/// Resolve the path `init` should write: explicit `-c` or cwd `machine_setup.yaml`.
pub fn resolve_init_path(config_arg: Option<&str>, cwd: &Path) -> PathBuf {
    match config_arg {
        Some(raw) if !raw.is_empty() => {
            let path = Path::new(raw);
            if path.extension().is_some() {
                path.to_path_buf()
            } else {
                path.with_extension("yaml")
            }
        }
        _ => locator::init_path(cwd),
    }
}

/// Create a new empty Config document. Refuses if the path already exists.
pub fn init(path: &Path) -> Result<()> {
    if path.exists() {
        return Err(Error::ConfigAlreadyExists(path.to_path_buf()));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let contents = format!(
        "# yaml-language-server: $schema={SCHEMA_MODELINE_URL}\n\
         # machine_setup Config document — add tasks with: machine_setup add task <name>\n\
         # See README.md for Command entry kinds (copy, symlink, clone, run, machine_setup).\n\
         \n\
         default_shell: bash\n\
         parallel: false\n\
         \n\
         tasks: {{}}\n"
    );
    std::fs::write(path, contents)?;
    Ok(())
}

/// Build a minimal blank Task for add/replace authoring.
pub fn emitted_blank_task(task_name: &str) -> Result<super::recipes::EmittedTask> {
    validate_task_name(task_name)?;
    Ok(super::recipes::EmittedTask {
        name: task_name.to_string(),
        task: blank_task_config(),
    })
}

/// Append a minimal Task stub. Requires an existing Config document. Refuses
/// duplicate Task names.
pub fn add_task(path: &Path, task_name: &str) -> Result<()> {
    append_emitted(path, &emitted_blank_task(task_name)?)
}

/// Serialize one task entry as a YAML fragment indented for nesting under `tasks:`.
pub fn format_task_yaml_fragment(name: &str, task: &TaskConfig) -> Result<String> {
    use indexmap::IndexMap;
    let mut map = IndexMap::new();
    map.insert(name.to_string(), task);
    let raw = serde_yaml::to_string(&map)?;
    let mut out = String::from("\n");
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

/// Append an emitted Task YAML block (from stubs or Authoring recipes).
pub fn append_emitted(path: &Path, emitted: &super::recipes::EmittedTask) -> Result<()> {
    if !path.is_file() {
        return Err(Error::ConfigNotFound(path.to_path_buf()));
    }

    let config = load_config(path.to_str().unwrap_or_default())?;
    if config.tasks.contains_key(&emitted.name) {
        return Err(Error::TaskAlreadyExists(emitted.name.clone()));
    }

    validate_task_name(&emitted.name)?;

    let mut content = std::fs::read_to_string(path)?;
    if !content.ends_with('\n') {
        content.push('\n');
    }

    if let Some(rewritten) = open_empty_tasks_map(&content) {
        content = rewritten;
    }

    content.push_str(&format_task_yaml_fragment(&emitted.name, &emitted.task)?);
    if !content.ends_with('\n') {
        content.push('\n');
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Load + semantic validate after an authoring write. Returns true if any Error
/// severity issues were found (caller should exit non-zero).
pub fn validate_after_write(path: &Path) -> Result<bool> {
    let config = load_config(
        path.to_str()
            .ok_or_else(|| Error::PathError(format!("invalid path: {}", path.display())))?,
    )?;
    let config_dir = resolve_config_dir(
        path.to_str().unwrap_or("."),
        path.parent().unwrap_or_else(|| Path::new(".")),
    );
    let issues = validate::validate_config(&config, &config_dir);
    let mut has_errors = false;
    for issue in &issues {
        println!(
            "[{}] {}: {}",
            issue.severity, issue.task_name, issue.message
        );
        if matches!(issue.severity, validate::Severity::Error) {
            has_errors = true;
        }
    }
    if issues.is_empty() {
        println!("Config is valid.");
    }
    Ok(has_errors)
}

pub fn validate_task_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidTaskName("must not be empty".into()));
    }
    if name.contains(':') || name.contains('#') || name.contains('\n') {
        return Err(Error::InvalidTaskName(format!(
            "contains invalid characters: {name:?}"
        )));
    }
    Ok(())
}

/// If `tasks: {}` (flow style empty), rewrite to `tasks:` so block entries can append.
fn open_empty_tasks_map(content: &str) -> Option<String> {
    let trimmed_end = content.trim_end();
    // Match last non-empty meaningful line ending with tasks: {}
    if let Some(idx) = trimmed_end.rfind("tasks:") {
        let after = trimmed_end[idx + "tasks:".len()..].trim();
        if after == "{}" {
            let mut out = trimmed_end[..idx].to_string();
            out.push_str("tasks:\n");
            return Some(out);
        }
    }
    None
}

/// Re-parse helper used by tests / callers that need the loaded config after write.
pub fn load_after_write(path: &Path) -> Result<AppConfig> {
    load_config(
        path.to_str()
            .ok_or_else(|| Error::PathError(format!("invalid path: {}", path.display())))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_creates_valid_empty_document() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        assert!(path.is_file());
        let config = load_after_write(&path).unwrap();
        assert!(config.tasks.is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("yaml-language-server"));
        assert!(text.contains(SCHEMA_MODELINE_URL));
    }

    #[test]
    fn init_refuses_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        let err = init(&path).unwrap_err();
        assert!(matches!(err, Error::ConfigAlreadyExists(_)));
    }

    #[test]
    fn add_task_appends_and_parses() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        add_task(&path, "dotfiles").unwrap();
        let config = load_after_write(&path).unwrap();
        assert!(config.tasks.contains_key("dotfiles"));
        assert!(config.tasks["dotfiles"].commands.is_empty());
    }

    #[test]
    fn add_task_refuses_duplicate() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        add_task(&path, "a").unwrap();
        let err = add_task(&path, "a").unwrap_err();
        assert!(matches!(err, Error::TaskAlreadyExists(_)));
    }

    #[test]
    fn add_task_requires_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.yaml");
        let err = add_task(&path, "a").unwrap_err();
        assert!(matches!(err, Error::ConfigNotFound(_)));
    }

    #[test]
    fn validate_after_write_ok_on_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        assert!(!validate_after_write(&path).unwrap());
    }

    #[test]
    fn blank_task_fragment_omits_default_fields_and_loads() {
        use crate::config::os::OsFilter;

        let fragment = format_task_yaml_fragment("stub", &blank_task_config()).unwrap();
        assert!(
            !fragment.contains("os: null"),
            "fragment must not contain os: null:\n{fragment}"
        );
        assert!(!fragment.contains("parallel: false"));
        assert!(!fragment.contains("depends_on:"));
        assert!(!fragment.contains("retry:"));

        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        init(&path).unwrap();
        add_task(&path, "stub").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("os: null"),
            "appended stub must not contain os: null:\n{text}"
        );
        let config = load_after_write(&path).unwrap();
        let task = &config.tasks["stub"];
        assert!(matches!(task.os, OsFilter::All));
        assert!(task.commands.is_empty());
        assert!(!task.parallel);
        assert!(task.depends_on.is_empty());
        assert_eq!(task.retry, 0);
        assert_eq!(task.retry_delay_secs, 1);
    }
}
