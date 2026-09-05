# Config document `replace` (typed emitters + upsert)

Date: 2026-09-05  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **3** — recipes/stubs emit typed `TaskConfig`; `add` appends serialized fragment; `replace` upserts via `document_edit`

## Context

ADR-0008 partially reopened for `remove task` (serde full-file rewrite in
`document_edit`). Upsert remained deferred. `add` / recipes still append
hand-built YAML strings (`EmittedTask { name, yaml }`).

We reopen upsert as a new **`replace`** verb (not by changing `add` into
overwrite). Approach 3 moves emitters to typed `TaskConfig` so both paths share
one content source.

## Goals

- CLI: `replace task <name>` and `replace recipe …` (same recipe flags as `add`).
- True upsert: missing name → create with a **warning**; existing → overwrite.
- Overwrite UX: TTY confirm; non-TTY overwrite with a **notice**. Preserve
  `IndexMap` key order on overwrite.
- Refactor `EmittedTask` to `{ name, task: TaskConfig }`; recipes and blank stub
  build `TaskConfig` in Rust.
- `add` keeps append-only semantics (refuse duplicates) but serializes the typed
  task to the YAML fragment instead of hand-written recipe strings.
- `replace` uses `document_edit` + serde `write_config` (YAML-only, same as remove).
- History unchanged on replace.
- Update ADR-0008, README, CHANGELOG.

## Non-goals

- Changing `add` to overwrite (still refuses duplicates).
- Wizard “replace” UI.
- Comment-preserving YAML surgery.
- JSON Config documents (refuse like remove).
- `--force` flag (TTY prompt / non-TTY auto is enough).
- Pruning or rewriting History on replace.

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| CLI shape | New `replace` verb; `add` unchanged at the UX level |
| Surfaces | `replace task` + `replace recipe` (mirror `add`) |
| Missing name | Create + warning (true upsert) |
| Existing name, TTY | Confirm overwrite; Abort → no write |
| Existing name, non-TTY | Overwrite + notice |
| Task order | Preserve position on overwrite |
| Emitter model | Typed `TaskConfig` (approach 3) |
| Rewrite | Serde full-file for `replace` only; `add` still appends a fragment |
| History | Leave alone |

## Architecture

```text
Recipe / stub params
        │
        ▼
EmittedTask { name, task: TaskConfig }
        │
        ├─ add ──────────► serialize fragment → append under tasks:
        │
        └─ replace ──────► document_edit::replace_task
                              load → confirm policy → IndexMap upsert
                              → write_config → validate_after_write (main)
```

## CLI

```text
machine_setup replace task <name>
machine_setup replace recipe <recipe> [recipe flags…]
```

- Global `-c` / locator same as `add` / `remove`.
- Recipe subcommands and flags identical to `add recipe`.

## Typed emitters

- Change `EmittedTask` to carry `TaskConfig` instead of a preformatted `yaml: String`.
- `emit_dotfiles` / `emit_git_repo` / `emit_brew_bundle` (and blank stub) construct
  equivalent `TaskConfig` graphs to today’s YAML.
- `document::append_emitted` turns `{name → task}` into an indented YAML block and
  appends (open empty `tasks: {}` behavior unchanged).
- Blank `add task` may keep leading comment lines in the fragment if useful;
  recipe fragments are serde-emitted (hand-tuned recipe comment blocks may differ).

## Replace behavior

1. Ensure YAML document (refuse `.json`).
2. Load `AppConfig`.
3. If name absent: insert at end; print create warning.
4. If name present:
   - TTY: dialoguer confirm; Abort → `Error::Aborted`.
   - Non-TTY: overwrite; print replaced notice.
   - Overwrite in place (same `IndexMap` position).
5. `write_config` (tasks last — already required for append compatibility).
6. Do not touch History.
7. Caller runs `validate_after_write`.

## Tests

- Recipe typed emit matches prior semantics (commands / os / fields).
- `add` with typed emit: append + load; duplicate refused.
- `replace` create path: task appears; order of others unchanged.
- `replace` overwrite: content updated; neighbor order preserved; abort leaves file unchanged.
- `replace recipe` overwrites; History entry retained if present.
- JSON path refused.

## Docs

- ADR-0008: `replace` / typed emitters shipped; comment-preserving surgery still deferred.
- README commands table + authoring notes (rewrite caveat like remove).
- CHANGELOG `[Unreleased]` Added.

## Follow-ups (out of scope)

- Wizard replace actions.
- Comment-preserving edit if serde rewrite pain becomes real.
