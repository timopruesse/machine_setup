# Replace task (typed emitters + upsert) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `machine_setup replace task|recipe` as a true upsert (create-with-warning / overwrite-with-TTY-confirm), backed by typed `TaskConfig` emitters shared with append-only `add`.

**Architecture:** Change `EmittedTask` to `{ name, task: TaskConfig }`. Recipes and blank stubs build `TaskConfig` in Rust. `document::append_emitted` serializes a YAML fragment and appends. `document_edit::replace_task` upserts in the `IndexMap` (preserving key order) then serde-rewrites the file. CLI mirrors `Add` under `Replace`.

**Tech Stack:** Rust, clap, serde_yaml, dialoguer, IndexMap, existing `document` / `document_edit` / `recipes`.

**Spec:** `docs/superpowers/specs/2026-09-05-replace-task-design.md`

## Global Constraints

- New `replace` verb; `add` still refuses duplicates (append-only).
- True upsert: missing → create + warning; existing → overwrite.
- TTY overwrite: confirm; Abort → no write. Non-TTY: overwrite + notice.
- Preserve `IndexMap` position on overwrite (`insert` on existing key).
- History untouched on replace.
- YAML-only for replace (same refuse as remove for `.json`).
- Wizard stays on `add` (no replace UI).
- No `--force` flag.
- Do not create git commits unless the user explicitly asks (skip commit steps).

## File map

| File | Responsibility |
| --- | --- |
| `src/config/types.rs` | Optional `From` helpers for `StringOrVec` / blank `TaskConfig` |
| `src/config/recipes.rs` | Typed `EmittedTask`; emitters return `TaskConfig` |
| `src/config/document.rs` | `append_emitted` serializes typed task; blank stub via `TaskConfig` |
| `src/config/document_edit.rs` | `replace_task` + confirm policy |
| `src/cli.rs` | `Command::Replace` / `ReplaceTarget` (mirror `Add`) |
| `src/main.rs` | Wire `Replace`; messages/warnings |
| `src/engine/mode.rs` | Exhaustiveness if `Command` match needs `Replace` |
| `docs/adr/0008-…` | Record replace shipped |
| `README.md` / `CHANGELOG.md` | Document replace |

---

### Task 1: Typed `EmittedTask` + recipe emitters + append path

**Files:**
- Modify: `src/config/recipes.rs`
- Modify: `src/config/document.rs`
- Modify: `src/config/types.rs` (small helpers only)

**Interfaces:**
- Produces:
  - `pub struct EmittedTask { pub name: String, pub task: TaskConfig }`
  - `emit_*` / `emit_from_cli` / `emit_by_key` return typed `EmittedTask`
  - `document::format_task_yaml_fragment(name, &TaskConfig) -> Result<String>` (or private)
  - `document::append_emitted` uses fragment serialization
  - `document::blank_task(name) -> EmittedTask` or inline in `add_task`
- Consumes: existing `CommandEntry` / `CloneArgs` / `SymlinkArgs` / `RunArgs` / `OsFilter`

- [ ] **Step 1: Add construction helpers** in `types.rs` if needed:

```rust
impl From<String> for StringOrVec {
    fn from(s: String) -> Self {
        StringOrVec(vec![s])
    }
}

impl From<&str> for StringOrVec {
    fn from(s: &str) -> Self {
        StringOrVec(vec![s.to_string()])
    }
}
```

Prefer a small `TaskConfig::empty()` or `fn blank_task_config() -> TaskConfig` with empty commands and field defaults (`retry_delay_secs: 1`, etc.) next to `TaskConfig` or in `document.rs`.

- [ ] **Step 2: Write failing tests** — update recipe tests to assert on `emitted.task` **before** append (and keep append round-trip):

```rust
#[test]
fn emit_git_repo_is_typed_clone() {
    let emitted = emit_git_repo(&GitRepoParams {
        name: "my-repo",
        url: "https://github.com/user/repo.git",
        target: "~/projects/repo",
    })
    .unwrap();
    assert_eq!(emitted.name, "my-repo");
    assert_eq!(emitted.task.commands.len(), 1);
    match &emitted.task.commands[0] {
        CommandEntry::Clone(a) => {
            assert_eq!(a.url, "https://github.com/user/repo.git");
            assert_eq!(a.target, "~/projects/repo");
        }
        other => panic!("expected Clone, got {other:?}"),
    }
}
```

Similar direct asserts for `emit_dotfiles` (clone+symlink, force, `.cursor` ignore) and `emit_brew_bundle` (macos + run install/update). Keep existing append+load tests; they must still pass after the refactor.

- [ ] **Step 3: Run tests — expect FAIL** (no `task` field yet)

Run: `cargo test --lib recipes::`
Expected: compile fail / FAIL

- [ ] **Step 4: Change `EmittedTask` and emitters**

```rust
pub struct EmittedTask {
    pub name: String,
    pub task: crate::config::types::TaskConfig,
}
```

`emit_git_repo` example:

```rust
pub fn emit_git_repo(p: &GitRepoParams<'_>) -> Result<EmittedTask> {
    crate::config::document::validate_task_name(p.name)?;
    let task = TaskConfig {
        commands: vec![CommandEntry::Clone(CloneArgs {
            url: p.url.to_string(),
            target: p.target.to_string(),
        })],
        // all other fields defaulted via helper / explicit defaults
        ..blank_task_config()
    };
    Ok(EmittedTask {
        name: p.name.to_string(),
        task,
    })
}
```

Mirror today’s semantics for dotfiles (clone target `"."`, symlink force + ignore with `.cursor` first) and brew-bundle (`os: macos`, run install+update with `brew bundle --file=…` using the same shell quoting as today).

- [ ] **Step 5: Update `document::append_emitted` / `add_task`**

Serialize a one-key map and indent under `tasks:`:

```rust
fn format_task_fragment(name: &str, task: &TaskConfig) -> Result<String> {
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
```

`append_emitted`: after duplicate check, `content.push_str(&format_task_fragment(&emitted.name, &emitted.task)?)`.

`add_task`: build `EmittedTask { name, task: blank_task_config() }` then `append_emitted`. Optional: prepend the existing comment block before the fragment for blank stubs only (nice-to-have; not required if serde-only is cleaner).

- [ ] **Step 6: Run tests — expect PASS**

Run: `cargo test --lib recipes:: document::`
Expected: PASS

- [ ] **Step 7: Commit** — skip unless user asks.

---

### Task 2: `document_edit::replace_task`

**Files:**
- Modify: `src/config/document_edit.rs`

**Interfaces:**
- Produces:
  - `pub enum OverwriteMode { Auto, /* prompt on TTY */ }` — or reuse a name like `ReplaceConfirm::{Prompt, Always}`
  - Prefer: `pub enum ReplaceMode { Auto }` where Auto = prompt if TTY + exists, else overwrite with notice; create always allowed
  - `pub fn apply_replace(config: &mut AppConfig, name: &str, task: TaskConfig) -> ReplaceOutcome`
  - `pub enum ReplaceOutcome { Created, Replaced }`
  - `pub fn replace_task(path: &Path, emitted: &EmittedTask, mode: ReplaceMode) -> Result<ReplaceOutcome>`
- Consumes: `write_config`, `ensure_yaml_document`, `load_config`, `dialoguer::Confirm`

- [ ] **Step 1: Write failing unit tests** in `document_edit.rs`:

```rust
#[test]
fn replace_creates_when_missing() {
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
    let cfg = document::load_after_write(&path).unwrap();
    assert!(cfg.tasks.contains_key("repo"));
}

#[test]
fn replace_overwrites_in_place_preserving_order() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("machine_setup.yaml");
    // Build file with tasks a, target, c via typed add
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
    // Non-TTY Auto overwrites without prompt
    replace_task(&path, &emitted, ReplaceMode::Auto).unwrap();
    let cfg = document::load_after_write(&path).unwrap();
    let keys: Vec<_> = cfg.tasks.keys().cloned().collect();
    assert_eq!(keys, vec!["a", "target", "c"]);
    assert!(matches!(
        cfg.tasks["target"].commands[0],
        CommandEntry::Clone(_)
    ));
}

#[test]
fn replace_refuses_json() {
    // same pattern as remove_task_refuses_json_config
}

#[test]
fn replace_does_not_prune_history() {
    // mark_installed for name; replace; History still contains name
}
```

For Abort-on-TTY: either skip (hard to simulate) or add `ReplaceMode::RefuseOverwrite` for tests that assert file unchanged when policy refuses — **not required by spec**. Spec’s Abort is TTY Confirm false → use injectable callback **only if** needed; otherwise document TTY path as manual/smoke. Prefer testing non-TTY overwrite + create paths thoroughly.

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test --lib document_edit::replace_`
Expected: FAIL / missing items

- [ ] **Step 3: Implement**

```rust
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

pub fn apply_replace(
    config: &mut AppConfig,
    name: &str,
    task: TaskConfig,
) -> ReplaceOutcome {
    let outcome = if config.tasks.contains_key(name) {
        ReplaceOutcome::Replaced
    } else {
        ReplaceOutcome::Created
    };
    // IndexMap::insert keeps position for existing keys
    config.tasks.insert(name.to_string(), Arc::new(task));
    outcome
}

pub fn replace_task(
    path: &Path,
    emitted: &crate::config::recipes::EmittedTask,
    mode: ReplaceMode,
) -> Result<ReplaceOutcome> {
    ensure_yaml_document(path)?;
    // reuse ensure_yaml message: generalize to "structural edits support YAML only"
    // or keep remove wording / share helper text "YAML config documents only"
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
                    let ok = dialoguer::Confirm::new()
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
```

Update `ensure_yaml_document` error string to something shared, e.g. `"structural edits currently support YAML config documents only"`.

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test --lib document_edit::`
Expected: PASS

- [ ] **Step 5: Commit** — skip unless user asks.

---

### Task 3: CLI + `main` wiring

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/engine/mode.rs` if `Command` exhaustiveness requires it

**Interfaces:**
- Produces: `Command::Replace { target: ReplaceTarget }` where `ReplaceTarget` mirrors `AddTarget` (`Task { name }` / `Recipe { recipe }`)
- Consumes: `recipes::emit_from_cli`, `document::add_task` pattern for blank → `EmittedTask`, `document_edit::replace_task`

- [ ] **Step 1: Add clap**

```rust
    /// Replace (upsert) a Task in the Config document (rewrites the file; comments/formatting may change)
    Replace {
        #[command(subcommand)]
        target: ReplaceTarget,
    },
```

```rust
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ReplaceTarget {
    Task { name: String },
    Recipe {
        #[command(subcommand)]
        recipe: RecipeCommand,
    },
}
```

- [ ] **Step 2: Wire `main.rs`** after Remove (mirror Add):

```rust
    if let Command::Replace { target } = &cli.command {
        let path = resolve_existing_document(cli.config.as_deref(), &cwd)?;
        let emitted = match target {
            ReplaceTarget::Task { name } => {
                // build blank EmittedTask (same as add_task content)
                config::document::emitted_blank_task(name)? // or inline
            }
            ReplaceTarget::Recipe { recipe } => config::recipes::emit_from_cli(recipe)?,
        };
        let outcome =
            config::document_edit::replace_task(&path, &emitted, config::document_edit::ReplaceMode::Auto)?;
        match outcome {
            ReplaceOutcome::Created => {
                eprintln!(
                    "warning: task `{}` did not exist; created it in {}",
                    emitted.name,
                    path.display()
                );
            }
            ReplaceOutcome::Replaced => {
                println!("Replaced task `{}` in {}", emitted.name, path.display());
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

For non-TTY replace-of-existing, still print the Replaced notice (spec). Create warning uses `eprintln!`.

Expose `document::emitted_blank_task(name) -> Result<EmittedTask>` so add/replace share one blank builder (refactor `add_task` to use it).

Update `resolve_existing_document` bail text to include `replace`.

- [ ] **Step 3: Help + lint**

Run: `cargo run -- replace task --help`  
Run: `cargo run -- replace recipe --help`  
Run: `make lint`  
Expected: clean; help shows task/recipe.

- [ ] **Step 4: Commit** — skip unless user asks.

---

### Task 4: Docs

**Files:**
- Modify: `docs/adr/0008-no-inplace-config-document-rewrite.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: ADR-0008** — note `replace` + typed emitters shipped; comment-preserving surgery still deferred; `add` remains append-only.

- [ ] **Step 2: README** — add `replace task` / `replace recipe` rows; note upsert warning, TTY confirm, file rewrite.

- [ ] **Step 3: CHANGELOG** — `[Unreleased]` Added entry for `replace` + typed emitter refactor (if user-visible).

- [ ] **Step 4: Commit** — skip unless user asks.

---

## Spec coverage (self-check)

| Spec requirement | Task |
| --- | --- |
| Typed `EmittedTask` / recipe emitters | 1 |
| `add` append via serialized fragment | 1 |
| `replace task` + `replace recipe` CLI | 3 |
| Create + warning / overwrite TTY+non-TTY | 2 + 3 |
| Preserve IndexMap order | 2 |
| History untouched | 2 |
| YAML-only | 2 |
| ADR / README / CHANGELOG | 4 |
| Wizard unchanged | honored |

## Placeholder scan

- No TBD. `ReplaceMode::Auto`, `ReplaceOutcome::{Created,Replaced}`, `EmittedTask.task` names stable across tasks.
- Blank task shared via `emitted_blank_task` between add and replace.
