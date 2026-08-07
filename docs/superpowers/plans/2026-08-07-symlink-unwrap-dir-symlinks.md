# Symlink unwrap intermediate dir symlinks — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Directory-mode `symlink` always unwraps intermediate destination directory symlinks into real directories so leaf links cannot write into the source tree (self-link bug).

**Architecture:** Replace tree-walk `mkdir` with `ensure_real_dir` in `symlink.rs` that detects symlinks via `symlink_metadata`, unlinks only the link inode, then creates a real directory. Add a self-link guard before creating leaf symlinks. Cover with an integration test that reproduces the nested leftover dir-symlink fixture.

**Tech Stack:** Rust, tokio, tempfile, existing `TaskRunner` integration harness.

**Spec:** `docs/superpowers/specs/2026-08-07-symlink-unwrap-dir-symlinks-design.md`

## Global Constraints

- No new YAML knobs.
- Unwrap intermediates always (independent of `force`); `force` only affects leaf replace/skip.
- Never `remove_dir_all` / `sudo_remove_dir` on a directory symlink (that would delete the source tree).
- Regular file where a directory is required → `PathError`, do not clobber.
- Dotfiles YAML cleanup is out of scope for this plan (follow-up after release).

## File map

| File | Responsibility |
| --- | --- |
| `src/engine/commands/symlink.rs` | `ensure_real_dir`, wire into install/update directory walks, self-link guard in `create_symlink` |
| `tests/integration.rs` | Regression: nested dir symlink under target must not corrupt source |
| `README.md` | Document intermediate-dir unwrap behavior under `symlink` |
| `CHANGELOG.md` | `[Unreleased]` Fixed entry |

---

### Task 1: Failing integration test

**Files:**
- Modify: `tests/integration.rs`

- [ ] Add `test_symlink_unwraps_nested_dir_symlink_without_corrupting_source` that:
  1. Creates temp `src/skills/route-agents/SKILL.md` with known content (e.g. `"route-agents-body"`).
  2. Creates temp `target/skills` as a real directory.
  3. Symlinks `target/skills/route-agents` → `src/skills/route-agents` (directory symlink).
  4. Writes a config that directory-symlinks `src` → `target` with `force: true`, using `with_config_dir` / absolute paths like existing `test_symlink_creation`.
  5. Runs install via `TaskRunner` with `force=true`.
  6. Asserts task completed successfully.
  7. Asserts `target/skills/route-agents` is a **real directory** (`symlink_metadata` is not a symlink / `is_symlink() == false`).
  8. Asserts `target/skills/route-agents/SKILL.md` is a symlink.
  9. Asserts source `SKILL.md` is still a regular file (`!is_symlink()`) with original content.
- [ ] Run `cargo test --test integration test_symlink_unwraps_nested_dir_symlink -- --nocapture` and confirm it **fails** (source corrupted or dest still a dir symlink) on current main.

### Task 2: Implement `ensure_real_dir` + wire directory walks

**Files:**
- Modify: `src/engine/commands/symlink.rs`

- [ ] Implement `ensure_real_dir(path, use_sudo, ctx) -> Result<()>`:
  - missing → create dir (`sudo_mkdir` / `create_dir_all`)
  - real directory → Ok
  - symlink → log unwrap, `sudo_remove` / `remove_file` (not `remove_dir_all`), then create dir
  - regular file → `Err(Error::PathError(...))`
- [ ] Use `ensure_real_dir` for `target` before the walk and for every directory entry in `walk_relative` (replace `mkdir` calls on the directory-walk path). Keep single-file mode using existing mkdir/`create_symlink` parent creation, or also use `ensure_real_dir` for parents if simpler and safe.
- [ ] In `create_symlink`, after force removal of an existing leaf (if any) and before creating the link: if `dest` still exists and `canonicalize(src) == canonicalize(dest)`, return `PathError` (self-link guard). Also apply when skipping is not taken.
- [ ] Run the Task 1 test until it passes; run full `cargo test` and fix regressions.

### Task 3: Docs

**Files:**
- Modify: `README.md` (symlink section)
- Modify: `CHANGELOG.md` (`[Unreleased]` Fixed)

- [ ] README: note that directory symlink walks always use real intermediate directories; leftover directory symlinks at dest are replaced by empty real dirs (link removed, pointed-to tree untouched).
- [ ] CHANGELOG Fixed bullet describing the self-link / nested dir-symlink fix.
- [ ] `cargo test` green once more.

### Task 4: Do not commit unless asked

- [ ] Leave changes unstaged/uncommitted unless the user explicitly asks to commit.
