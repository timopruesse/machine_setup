//! Structural Config document edits (serde rewrite).
//!
//! Append-only authoring stays in `document`. Upsert via `replace_task`.

use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;

use dialoguer::{Confirm, Select};

use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::document;
use super::graph::TaskGraph;
use super::history::History;
use super::load_config;
use super::types::{AppConfig, TaskConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixDepsMode {
    /// Prompt on TTY; `RemoveBlocked` when non-TTY and dependents exist.
    Auto,
    /// Strip `depends_on` edges then remove (no prompt).
    Force,
}

pub fn dependents_of(config: &AppConfig, name: &str) -> Vec<String> {
    let graph = TaskGraph::new(&config.tasks);
    graph
        .dependents_outside(&[name.to_string()])
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, deps)| deps)
        .unwrap_or_default()
}

pub fn apply_remove(config: &mut AppConfig, name: &str, strip_deps: bool) -> Result<()> {
    if !config.tasks.contains_key(name) {
        return Err(Error::TaskNotFound(name.to_string()));
    }
    let deps = dependents_of(config, name);
    if !deps.is_empty() && !strip_deps {
        return Err(Error::RemoveBlocked {
            task: name.to_string(),
            dependents: deps.join(", "),
        });
    }
    if strip_deps {
        for dependent in &deps {
            if let Some(task) = config.tasks.get_mut(dependent) {
                Arc::make_mut(task).depends_on.retain(|d| d != name);
            }
        }
    }
    config.tasks.shift_remove(name);
    Ok(())
}

const YAML_ONLY_STRUCTURAL_EDITS: &str =
    "structural edits currently support YAML config documents only";

fn ensure_yaml_document(path: &Path) -> Result<()> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return Err(Error::PathError(YAML_ONLY_STRUCTURAL_EDITS.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum ReplaceMode {
    /// Create always; if exists: Confirm on TTY, else overwrite.
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceOutcome {
    Created,
    Replaced,
}

pub fn apply_replace(config: &mut AppConfig, name: &str, task: TaskConfig) -> ReplaceOutcome {
    let outcome = if config.tasks.contains_key(name) {
        ReplaceOutcome::Replaced
    } else {
        ReplaceOutcome::Created
    };
    config.tasks.insert(name.to_string(), Arc::new(task));
    outcome
}

pub fn replace_task(
    path: &Path,
    emitted: &crate::config::recipes::EmittedTask,
    mode: ReplaceMode,
) -> Result<ReplaceOutcome> {
    ensure_yaml_document(path)?;
    if !path.is_file() {
        return Err(Error::ConfigNotFound(path.to_path_buf()));
    }
    document::validate_task_name(&emitted.name)?;
    let mut config = load_config(path.to_str().unwrap_or_default())?;
    let exists = config.tasks.contains_key(&emitted.name);
    if exists {
        match mode {
            ReplaceMode::Auto => {
                if std::io::stdin().is_terminal() {
                    let ok = Confirm::new()
                        .with_prompt(format!(
                            "Task `{}` already exists. Replace it?",
                            emitted.name
                        ))
                        .default(false)
                        .interact()
                        .map_err(|e| Error::PromptFailed(e.to_string()))?;
                    if !ok {
                        return Err(Error::Aborted);
                    }
                }
            }
        }
    }
    let outcome = apply_replace(&mut config, &emitted.name, emitted.task.clone());
    write_config(path, &config)?;
    Ok(outcome)
}

pub fn write_config(path: &Path, config: &AppConfig) -> Result<()> {
    ensure_yaml_document(path)?;
    let yaml = serde_yaml::to_string(config)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

pub fn prune_history(temp_dir: &Path, name: &str) -> Result<()> {
    let mut history = History::load(temp_dir).unwrap_or_default();
    if history.tasks.remove(name).is_some() {
        history.save(temp_dir)?;
    }
    Ok(())
}

fn prompt_fix_deps(dependents: &[String]) -> Result<bool> {
    eprintln!(
        "Task is depended on by: {}. Auto-fix (strip depends_on) or abort?",
        dependents.join(", ")
    );
    let choice = Select::new()
        .items(["Auto-fix dependent tasks", "Abort"])
        .default(1)
        .interact()
        .map_err(|e| Error::PromptFailed(e.to_string()))?;
    Ok(choice == 0)
}

/// Remove a Task from the Config document at `path`.
pub fn remove_task(path: &Path, name: &str, mode: FixDepsMode) -> Result<()> {
    if !path.is_file() {
        return Err(Error::ConfigNotFound(path.to_path_buf()));
    }
    ensure_yaml_document(path)?;
    let mut config = load_config(path.to_str().unwrap_or_default())?;
    if !config.tasks.contains_key(name) {
        return Err(Error::TaskNotFound(name.to_string()));
    }

    let deps = dependents_of(&config, name);
    let strip = if deps.is_empty() {
        false
    } else {
        match mode {
            FixDepsMode::Force => true,
            FixDepsMode::Auto => {
                if std::io::stdin().is_terminal() {
                    if !prompt_fix_deps(&deps)? {
                        return Err(Error::Aborted);
                    }
                    true
                } else {
                    return Err(Error::RemoveBlocked {
                        task: name.to_string(),
                        dependents: deps.join(", "),
                    });
                }
            }
        }
    };

    apply_remove(&mut config, name, strip)?;
    write_config(path, &config)?;

    let temp_dir = expand_path(&config.temp_dir, None);
    prune_history(&temp_dir, name)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::document::{self, load_after_write};
    use crate::config::history::History;
    use crate::error::Error;
    use tempfile::tempdir;

    fn write_yaml(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn apply_remove_drops_task() {
        let mut cfg: AppConfig = serde_yaml::from_str(
            r#"
tasks:
  a:
    commands: []
  b:
    commands: []
"#,
        )
        .unwrap();
        apply_remove(&mut cfg, "a", false).unwrap();
        assert!(!cfg.tasks.contains_key("a"));
        assert!(cfg.tasks.contains_key("b"));
    }

    #[test]
    fn apply_remove_refuses_dependents_without_strip() {
        let mut cfg: AppConfig = serde_yaml::from_str(
            r#"
tasks:
  base:
    commands: []
  child:
    depends_on: [base]
    commands: []
"#,
        )
        .unwrap();
        let err = apply_remove(&mut cfg, "base", false).unwrap_err();
        assert!(matches!(err, Error::RemoveBlocked { .. }));
        assert!(cfg.tasks.contains_key("base"));
    }

    #[test]
    fn apply_remove_strips_deps_when_requested() {
        let mut cfg: AppConfig = serde_yaml::from_str(
            r#"
tasks:
  base:
    commands: []
  child:
    depends_on: [base]
    commands: []
"#,
        )
        .unwrap();
        apply_remove(&mut cfg, "base", true).unwrap();
        assert!(!cfg.tasks.contains_key("base"));
        assert!(cfg.tasks["child"].depends_on.is_empty());
    }

    #[test]
    fn remove_task_force_rewrites_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        write_yaml(
            &path,
            r#"
default_shell: bash
parallel: false
tasks:
  gone:
    commands: []
  keep:
    depends_on: [gone]
    commands: []
"#,
        );
        remove_task(&path, "gone", FixDepsMode::Force).unwrap();
        let cfg = load_after_write(&path).unwrap();
        assert!(!cfg.tasks.contains_key("gone"));
        assert!(cfg.tasks["keep"].depends_on.is_empty());
    }

    #[test]
    fn remove_task_auto_non_tty_blocks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        write_yaml(
            &path,
            r#"
tasks:
  gone:
    commands: []
  keep:
    depends_on: [gone]
    commands: []
"#,
        );
        let before = std::fs::read_to_string(&path).unwrap();
        let err = remove_task(&path, "gone", FixDepsMode::Auto).unwrap_err();
        assert!(matches!(err, Error::RemoveBlocked { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn remove_task_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let err = remove_task(&path, "nope", FixDepsMode::Force).unwrap_err();
        assert!(matches!(err, Error::TaskNotFound(_)));
    }

    #[test]
    fn remove_task_prunes_history() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        let hist_dir = dir.path().join("hist");
        std::fs::create_dir_all(&hist_dir).unwrap();
        write_yaml(
            &path,
            &format!(
                "temp_dir: {}\ntasks:\n  gone:\n    commands: []\n  keep:\n    commands: []\n",
                hist_dir.display()
            ),
        );
        let mut h = History::default();
        h.mark_installed("gone");
        h.mark_installed("keep");
        h.save(&hist_dir).unwrap();

        remove_task(&path, "gone", FixDepsMode::Force).unwrap();

        let h = History::load(&hist_dir).unwrap();
        assert!(!h.tasks.contains_key("gone"));
        assert!(h.tasks.contains_key("keep"));
    }

    #[test]
    fn remove_then_add_task_appends_valid_yaml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        document::add_task(&path, "task_a").unwrap();
        remove_task(&path, "task_a", FixDepsMode::Force).unwrap();
        document::add_task(&path, "task_b").unwrap();
        let cfg = load_after_write(&path).unwrap();
        assert!(!cfg.tasks.contains_key("task_a"));
        assert!(cfg.tasks.contains_key("task_b"));
    }

    #[test]
    fn remove_task_refuses_json_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.json");
        let json = r#"{"tasks":{"gone":{"commands":[]}}}"#;
        std::fs::write(&path, json).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let err = remove_task(&path, "gone", FixDepsMode::Force).unwrap_err();
        assert!(matches!(err, Error::PathError(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn replace_creates_when_missing() {
        use crate::config::recipes::{self, GitRepoParams};

        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        let emitted = recipes::emit_git_repo(&GitRepoParams {
            name: "repo",
            url: "https://example.com/r.git",
            target: "~/r",
        })
        .unwrap();
        let outcome = replace_task(&path, &emitted, ReplaceMode::Auto).unwrap();
        assert!(matches!(outcome, ReplaceOutcome::Created));
        let cfg = load_after_write(&path).unwrap();
        assert!(cfg.tasks.contains_key("repo"));
    }

    #[test]
    fn replace_overwrites_in_place_preserving_order() {
        use crate::config::recipes::{self, GitRepoParams};
        use crate::config::types::CommandEntry;

        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        document::init(&path).unwrap();
        document::add_task(&path, "a").unwrap();
        document::add_task(&path, "target").unwrap();
        document::add_task(&path, "c").unwrap();
        let emitted = recipes::emit_git_repo(&GitRepoParams {
            name: "target",
            url: "https://example.com/new.git",
            target: "~/new",
        })
        .unwrap();
        replace_task(&path, &emitted, ReplaceMode::Auto).unwrap();
        let cfg = load_after_write(&path).unwrap();
        let keys: Vec<_> = cfg.tasks.keys().cloned().collect();
        assert_eq!(keys, vec!["a", "target", "c"]);
        assert!(matches!(
            cfg.tasks["target"].commands[0],
            CommandEntry::Clone(_)
        ));
    }

    #[test]
    fn replace_refuses_json() {
        use crate::config::recipes::{self, GitRepoParams};

        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.json");
        let json = r#"{"tasks":{"gone":{"commands":[]}}}"#;
        std::fs::write(&path, json).unwrap();
        let emitted = recipes::emit_git_repo(&GitRepoParams {
            name: "gone",
            url: "https://example.com/r.git",
            target: "~/r",
        })
        .unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let err = replace_task(&path, &emitted, ReplaceMode::Auto).unwrap_err();
        assert!(matches!(err, Error::PathError(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn replace_does_not_prune_history() {
        use crate::config::recipes::{self, GitRepoParams};

        let dir = tempdir().unwrap();
        let path = dir.path().join("machine_setup.yaml");
        let hist_dir = dir.path().join("hist");
        std::fs::create_dir_all(&hist_dir).unwrap();
        write_yaml(
            &path,
            &format!(
                "temp_dir: {}\ntasks:\n  target:\n    commands: []\n",
                hist_dir.display()
            ),
        );
        let mut h = History::default();
        h.mark_installed("target");
        h.save(&hist_dir).unwrap();

        let emitted = recipes::emit_git_repo(&GitRepoParams {
            name: "target",
            url: "https://example.com/new.git",
            target: "~/new",
        })
        .unwrap();
        replace_task(&path, &emitted, ReplaceMode::Auto).unwrap();

        let h = History::load(&hist_dir).unwrap();
        assert!(h.tasks.contains_key("target"));
    }
}
