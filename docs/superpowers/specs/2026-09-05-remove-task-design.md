# Config document `remove task` (serde rewrite)

Date: 2026-09-05  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **2** — separate `document_edit` module; append-only `document.rs` unchanged

## Context

ADR-0008 deferred in-place Config document Task rewrite: authoring stayed
create (`init`) and append (`add task`, recipes, wizard). Users edit or replace
YAML by hand for deletes.

We are reopening the ADR for **remove only**. Upsert stays deferred. Full-file
serde round-trip is acceptable: comments and hand formatting may change.

## Goals

- CLI: `machine_setup remove task <name> [--fix-deps]`.
- New module `src/config/document_edit.rs` for structural edits; `document.rs`
  remains create/append-only.
- Refuse remove while other Tasks still `depends_on` the target, unless the
  user chooses auto-fix (TTY) or passes `--fix-deps` (non-TTY / scripted).
- On success: rewrite Config via `serde_yaml`, prune that Task from History.
- Unit tests for happy path, dependents abort/fix, missing Task, History prune.
- Update ADR-0008 to record remove as shipped and upsert still deferred.
- Document that remove rewrites the file (comments/formatting may be lost).

## Non-goals

- Upsert / replace Task blocks in place.
- Comment-preserving YAML surgery.
- Wizard UI for remove.
- Editing nested configs included via `machine_setup` Command entries.
- Changing `add` / `init` / recipe append behavior.

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Scope | Remove only (not upsert) |
| Rewrite strategy | Serde load → mutate → `serde_yaml` dump (comments/format OK to lose) |
| Module layout | `document_edit.rs` separate from append-only `document.rs` |
| Dependents (TTY) | Warn + dialoguer: Auto-fix (strip deps then remove) or Abort; never remove with unresolved deps |
| Dependents (non-TTY) | Abort listing dependents; require `--fix-deps` to strip and remove |
| History | Prune removed Task name on successful remove |
| Config path | Same locator / `-c` as `add` |
| Post-write | Existing `validate_after_write` |

## Architecture

```text
CLI Remove::Task { name, fix_deps }
        │
        ▼
config locator / -c  →  path
        │
        ▼
document_edit::remove_task(path, name, FixDeps::{Prompt, Force, …})
        │
        ├─ load AppConfig
        ├─ Task missing → error
        ├─ find dependents (other tasks' depends_on contains name)
        ├─ if dependents:
        │     Prompt → dialoguer Auto-fix | Abort
        │     Force  → strip deps
        │     Abort-only (non-TTY without flag) → error
        ├─ strip deps (if fixing) + remove task from IndexMap
        ├─ serde_yaml::to_string → write path
        ├─ History: remove name
        └─ validate_after_write
```

`document.rs` stays responsible for `init`, `add_task`, `append_emitted`. No
shared “mutate then dump” helpers leak into the append path.

## CLI

```text
machine_setup remove task <name> [--fix-deps]
```

- New clap subtree symmetric with `Add` / `AddTarget::Task`.
- Global `-c` / config search unchanged.
- Missing Task → non-zero exit.
- `--fix-deps` with no dependents is a no-op (still removes).

## Dependents behavior

**Detection:** any Task other than `name` whose `depends_on` list contains
`name`.

| Environment | Behavior |
| --- | --- |
| No dependents | Remove without prompt |
| TTY + dependents | Warn with names; Select/Confirm: Auto-fix or Abort |
| Non-TTY + dependents, no flag | Error listing dependents; mention `--fix-deps` |
| `--fix-deps` | Strip `name` from each dependent’s `depends_on`, then remove |

Abort paths must not write Config or History.

## Serialization & History

- Dump with existing `Serialize` impls (`AppConfig`, `TaskConfig`, `IndexMap`
  key order where applicable). No custom pretty-printer in v1.
- History prune uses the same store as `doctor --fix` orphan cleanup; only the
  removed Task name is dropped.

## Errors

- Task not found.
- Remove aborted (user Abort, or non-TTY without `--fix-deps` when dependents exist).
- I/O / serialize failures.

## Tests

Unit tests with tempfiles:

1. Remove sole / one of several Tasks — remaining map correct after reload.
2. Dependents + abort — file unchanged.
3. Dependents + fix — deps stripped, Task gone.
4. Non-interactive `--fix-deps` path.
5. Missing Task → error.
6. History entry for removed name gone; others retained.

## Docs

- ADR-0008: reopen for remove; note serde rewrite; upsert still deferred.
- README / `--help`: remove rewrites the Config document.

## Follow-ups (out of scope)

- Upsert into `document_edit` when a concrete need appears.
- Comment-preserving edit if round-trip pain becomes real.
