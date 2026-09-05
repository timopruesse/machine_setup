# Remove task (document_edit) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `machine_setup remove task <name> [--fix-deps]` that deletes a Task from the Config document via serde rewrite, with dependent-task gating and History prune.

**Architecture:** New `src/config/document_edit.rs` owns structural edits. Append-only `document.rs` stays untouched. Before dump works safely, `CommandEntry` (and `Condition`) need custom `Serialize` matching their existing `Deserialize` shapes — derived enum tags are not loadable YAML.

**Tech Stack:** Rust, clap, serde_yaml, dialoguer, IndexMap, existing `TaskGraph` / `History` / `validate_after_write`.

**Spec:** `docs/superpowers/specs/2026-09-05-remove-task-design.md`

## Global Constraints

- Remove only — no upsert.
- Serde full-file rewrite; comments/formatting may change (accepted).
- Structural edits live in `document_edit.rs`, not `document.rs`.
- Never write Config/History on Abort / blocked dependents.
- TTY: dialoguer Auto-fix vs Abort; non-TTY: require `--fix-deps`.
- Prune History for the removed Task name on success.
- Reuse `Error::TaskNotFound`, `Error::Aborted`; add `Error::RemoveBlocked` for non-TTY dependents.
- Do not create git commits unless the user explicitly asks (skip commit steps).

## File map

| File | Responsibility |
| --- | --- |
| `src/config/types.rs` | Custom `Serialize` for `CommandEntry` + `Condition` so dump ↔ load |
| `src/config/document_edit.rs` | `remove_task`, dependents policy, mutate, write, History prune |
| `src/config/mod.rs` | `pub mod document_edit` |
| `src/error.rs` | `RemoveBlocked { task, dependents }` |
| `src/cli.rs` | `Command::Remove` / `RemoveTarget::Task { name, fix_deps }` |
| `src/main.rs` | Wire `Remove` like `Add` |
| `docs/adr/0008-no-inplace-config-document-rewrite.md` | Reopen for remove; upsert still deferred |
| `README.md` | Document `remove task` + rewrite caveat |
| `CHANGELOG.md` | `[Unreleased]` Added |

---

### Task 1: YAML emit fidelity (`CommandEntry` / `Condition` Serialize)

**Files:**
- Modify: `src/config/types.rs` (replace derived `Serialize` on `CommandEntry` and `Condition` with hand-written impls; keep `Conditions` / `StringOrVec` working)
- Test: `src/config/types.rs` `#[cfg(test)]`

**Interfaces:**
- Produces: `CommandEntry` and `Condition` serialize to the same YAML shapes `Deserialize` already accepts (`copy:` / `symlink:` / …; path string or `{path|env|command|mode: …}`).
- Consumes: existing arg structs (`CopyArgs`, …) and `Mode`.

- [ ] **Step 1: Write failing round-trip tests** in `types.rs` tests module:

```rust
#[test]
fn command_entry_yaml_roundtrip_copy() {
    let yaml = r#"
- copy:
    src: ./a
    target: ~/b
"#;
    let entries: Vec<CommandEntry> = serde_yaml::from_str(yaml).unwrap();
    let out = serde_yaml::to_string(&entries).unwrap();
    let again: Vec<CommandEntry> = serde_yaml::from_str(&out).unwrap();
    assert!(matches!(again[0], CommandEntry::Copy(_)));
}

#[test]
fn command_entry_yaml_roundtrip_run_symlink_clone() {
    let yaml = r#"
- run:
    commands: "echo hi"
- symlink:
    src: ./x
    target: ~/x
    force: true
- clone:
    url: https://example.com/r.git
    target: ~/r
"#;
    let entries: Vec<CommandEntry> = serde_yaml::from_str(yaml).unwrap();
    let out = serde_yaml::to_string(&entries).unwrap();
    let again: Vec<CommandEntry> = serde_yaml::from_str(&out).unwrap();
    assert_eq!(again.len(), 3);
}

#[test]
fn conditions_yaml_roundtrip() {
    let yaml = r#"
- ~/.ssh
- env: HOME
- command: "true"
- mode: install
"#;
    let c: Conditions = serde_yaml::from_str(yaml).unwrap();
    let out = serde_yaml::to_string(&c).unwrap();
    let again: Conditions = serde_yaml::from_str(&out).unwrap();
    assert_eq!(c, again);
}
```

- [ ] **Step 2: Run tests — expect FAIL** (derived `CommandEntry` / `Condition` emit `!Copy` / `!Path` tags that do not deserialize)

Run: `cargo test --lib command_entry_yaml_roundtrip -- --nocapture`
Expected: FAIL on re-parse (or assert).

- [ ] **Step 3: Implement `Serialize` for `CommandEntry`**

Remove `Serialize` from the `CommandEntry` derive. Add:

```rust
impl Serialize for CommandEntry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            CommandEntry::Copy(a) => map.serialize_entry("copy", a)?,
            CommandEntry::Symlink(a) => map.serialize_entry("symlink", a)?,
            CommandEntry::Clone(a) => map.serialize_entry("clone", a)?,
            CommandEntry::Run(a) => map.serialize_entry("run", a)?,
            CommandEntry::MachineSetup(a) => map.serialize_entry("machine_setup", a)?,
        }
        map.end()
    }
}
```

- [ ] **Step 4: Implement `Serialize` for `Condition`**

Remove `Serialize` from the `Condition` derive. Emit maps matching `ConditionObj` / path strings:

```rust
impl Serialize for Condition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            Condition::Path(p) => serializer.serialize_str(p),
            Condition::Env(e) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("env", e)?;
                map.end()
            }
            Condition::Command(c) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("command", c)?;
                map.end()
            }
            Condition::Mode(modes) => {
                let mut map = serializer.serialize_map(Some(1))?;
                if modes.len() == 1 {
                    map.serialize_entry("mode", &modes[0])?;
                } else {
                    map.serialize_entry("mode", modes)?;
                }
                map.end()
            }
        }
    }
}
```

Keep `#[derive(Serialize)]` on `Conditions` (sequence of `Condition`).

- [ ] **Step 5: Run tests — expect PASS**

Run: `cargo test --lib command_entry_yaml_roundtrip conditions_yaml_roundtrip`
Expected: PASS

- [ ] **Step 6: Commit** — skip unless user asks.

---

### Task 2: `document_edit` core — mutate + write + History

**Files:**
- Create: `src/config/document_edit.rs`
- Modify: `src/config/mod.rs` — add `pub mod document_edit;`
- Modify: `src/error.rs` — add `RemoveBlocked`

**Interfaces:**
- Consumes: `load_config`, `TaskGraph::dependents_outside`, `History`, `expand_path`, `document::validate_after_write`
- Produces:
  - `pub enum FixDepsMode { Auto, Force }`
  - `pub fn dependents_of(config: &AppConfig, name: &str) -> Vec<String>`
  - `pub fn apply_remove(config: &mut AppConfig, name: &str, strip_deps: bool) -> Result<()>`
  - `pub fn write_config(path: &Path, config: &AppConfig) -> Result<()>`
  - `pub fn prune_history(temp_dir: &Path, name: &str) -> Result<()>`
  - `pub fn remove_task(path: &Path, name: &str, mode: FixDepsMode) -> Result<()>`

- [ ] **Step 1: Add error variant** in `src/error.rs`:

```rust
#[error(
    "Cannot remove task '{task}': still depended on by: {dependents}. \
     Re-run with `--fix-deps` to strip those edges, or run interactively to choose."
)]
RemoveBlocked { task: String, dependents: String },
```

(`dependents` = comma-separated sorted names.)

- [ ] **Step 2: Write failing unit tests** in `document_edit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::document::{self, load_after_write};
    use crate::config::history::History;
    use crate::error::Error;
    use crate::utils::path::expand_path;
    use std::sync::Arc;
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
        // Auto without TTY must not write — tests run non-interactive.
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
}
```

Adjust `temp_dir` YAML if path needs quoting (use a relative `./hist` under the tempfile and set `temp_dir` accordingly so `expand_path` resolves).

- [ ] **Step 3: Run tests — expect FAIL** (module missing)

Run: `cargo test --lib document_edit::`
Expected: compile fail / FAIL

- [ ] **Step 4: Implement module**

```rust
//! Structural Config document edits (serde rewrite).
//!
//! Append-only authoring stays in `document`. Upsert is still deferred (ADR-0008).

use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;

use dialoguer::Select;

use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::graph::TaskGraph;
use super::history::History;
use super::types::{AppConfig, TaskConfig};
use super::{document, load_config};

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
        for dep_name in &deps {
            if let Some(task) = config.tasks.get_mut(dep_name) {
                Arc::make_mut(task)
                    .depends_on
                    .retain(|d| d != name);
            }
        }
    }
    config.tasks.shift_remove(name);
    Ok(())
}

pub fn write_config(path: &Path, config: &AppConfig) -> Result<()> {
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
        .items(&["Auto-fix dependent tasks", "Abort"])
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
                    if !prompt_fix_deps(&deps) {
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

    let _ = document::validate_after_write(path)?;
    Ok(())
}
```

Notes for the implementer:
- `TaskGraph::new` — use the existing constructor signature in `graph.rs` (adjust if named differently).
- Prefer `shift_remove` / `swap_remove` matching `IndexMap` API in this crate’s version.
- `apply_remove` when `strip_deps && deps.is_empty()` still removes the task.
- Do not call `validate_after_write`’s bool as a hard error inside `remove_task` unless CLI wants exit 1 — match `add` (CLI checks the bool). Prefer having `remove_task` return `Result<()>` and let CLI call `validate_after_write` separately like `Add` does — **align with `main.rs` Add pattern**: mutation in module, validate in `main`. Then drop `validate_after_write` from `remove_task` and only write + prune History there.

**Preferred final signature (match Add):**

```rust
pub fn remove_task(path: &Path, name: &str, mode: FixDepsMode) -> Result<()> {
    // load, policy, apply_remove, write_config, prune_history — no validate here
}
```

CLI calls `document::validate_after_write` after success.

Update tests accordingly (remove validate coupling).

- [ ] **Step 5: Run tests — expect PASS**

Run: `cargo test --lib document_edit::`
Expected: PASS

- [ ] **Step 6: Commit** — skip unless user asks.

---

### Task 3: CLI + `main` wiring

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `document_edit::{remove_task, FixDepsMode}`, `resolve_existing_document`, `document::validate_after_write`
- Produces: `Command::Remove { target: RemoveTarget }`

- [ ] **Step 1: Extend clap**

In `src/cli.rs`, add next to `Add`:

```rust
    /// Remove from the Config document (rewrites the file; comments/formatting may change)
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },
```

```rust
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum RemoveTarget {
    /// Delete a Task by name
    Task {
        /// Task name
        name: String,
        /// Strip this name from other Tasks' depends_on, then remove (required non-interactively when dependents exist)
        #[arg(long)]
        fix_deps: bool,
    },
}
```

- [ ] **Step 2: Wire `main.rs`** after the `Add` block:

```rust
    if let Command::Remove { target } = &cli.command {
        let path = resolve_existing_document(cli.config.as_deref(), &cwd)?;
        match target {
            RemoveTarget::Task { name, fix_deps } => {
                let mode = if *fix_deps {
                    config::document_edit::FixDepsMode::Force
                } else {
                    config::document_edit::FixDepsMode::Auto
                };
                config::document_edit::remove_task(&path, name, mode)?;
                println!("Removed task `{name}` from {}", path.display());
            }
        }
        if config::document::validate_after_write(&path)? {
            notice.emit(&cli.command);
            std::process::exit(1);
        }
        notice.emit(&cli.command);
        return Ok(());
    }
```

Import `RemoveTarget`. Update `resolve_existing_document` bail message to mention `remove` as well as `add` (e.g. `` `add`/`remove` require a local Config document path ``).

- [ ] **Step 3: Smoke-check help**

Run: `cargo run -- remove task --help`
Expected: shows `<NAME>` and `--fix-deps`.

- [ ] **Step 4: Lint**

Run: `make lint`
Expected: clean.

- [ ] **Step 5: Commit** — skip unless user asks.

---

### Task 4: Docs (ADR, README, CHANGELOG)

**Files:**
- Modify: `docs/adr/0008-no-inplace-config-document-rewrite.md`
- Modify: `README.md` (commands table + authoring paragraph)
- Modify: `CHANGELOG.md` under `[Unreleased]`

- [ ] **Step 1: Rewrite ADR-0008** to record the reopen:

```markdown
# Config document structural edits

## Status

**Partially reopened (2026-09-05):** `remove task` ships via serde full-file
rewrite in `config::document_edit`. Append-only authoring remains in
`config::document` (`init`, `add task`, recipes, wizard).

**Still deferred:** upsert / replace of an existing Task block; comment-preserving
YAML surgery.

## Decision

- **Remove:** load → mutate `AppConfig` → `serde_yaml` dump → prune History.
  Dependents must be auto-fixed (TTY prompt or `--fix-deps`) or the remove aborts.
- **Upsert:** still deferred until a concrete need appears.
- **Append path:** unchanged; does not round-trip the file.

## Consequences

`remove task` may drop comments and reformat the Config document. Users who care
about hand-tuned YAML should edit by hand or avoid `remove`.
```

- [ ] **Step 2: README** — add row:

`| remove task | delete a Task (rewrites file; may drop comments) | `machine_setup remove task tools` |`

Note `--fix-deps` and that History for that Task is pruned.

- [ ] **Step 3: CHANGELOG** — under `[Unreleased]` → `### Added`:

```markdown
- `remove task <name> [--fix-deps]` — delete a Task via Config rewrite; prompts (or `--fix-deps`) when dependents exist; prunes History
```

- [ ] **Step 4: Commit** — skip unless user asks.

---

## Spec coverage (self-check)

| Spec requirement | Task |
| --- | --- |
| CLI `remove task` + `--fix-deps` | 3 |
| `document_edit` module; `document` append-only | 2 |
| Serde rewrite | 1 (fidelity) + 2 (write) |
| TTY prompt / non-TTY `--fix-deps` / Abort | 2 |
| History prune | 2 |
| `validate_after_write` | 3 (CLI, like Add) |
| ADR-0008 / README / rewrite caveat | 4 |
| Unit tests listed in spec | 2 |
| Upsert out of scope | honored |
| CommandEntry dump actually loadable | 1 (required for approach C) |

## Placeholder / consistency scan

- No TBD. `FixDepsMode::{Auto,Force}` names stable across tasks.
- `RemoveBlocked` + `Aborted` cover non-TTY vs user Abort.
- Validate stays in `main` to mirror `Add`.
