# Self update-check notice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or subagent-driven-development.

**Goal:** After most CLI commands, print a stderr notice when a newer GitHub release exists, with an install-method-specific update command.

**Architecture:** New `src/update_check/` module (cache, detect, fetch, version + entrypoint). `main` calls `maybe_print_update_notice` before exit on non-skipped verbs. Config `check_for_updates` + env `MACHINE_SETUP_NO_UPDATE_CHECK`.

**Tech Stack:** Rust, ureq, chrono, serde_json; no new crates (hand-rolled X.Y.Z compare).

**Spec:** `docs/superpowers/specs/2026-08-15-self-update-check-design.md`

## Global Constraints

- Fail open; never change CLI success/failure for network errors
- Skip: `completions`, `schedule notify`, `schema`
- Cache TTL 24h under `{temp_dir}/update_check.json`
- Stderr notice only; after TUI teardown
- Heuristic install detection from `current_exe()`

## File map

| File | Role |
| --- | --- |
| `src/update_check/{mod,cache,detect,fetch,version}.rs` | Feature |
| `src/config/types.rs` + schema | `check_for_updates` |
| `src/main.rs` + `src/lib.rs` | Wire + export |
| `README.md` / `CHANGELOG.md` | Docs |

### Task 1: version + detect + cache (unit tested)
### Task 2: fetch parse + maybe_print entrypoint
### Task 3: AppConfig + schema + main wiring
### Task 4: README/CHANGELOG + lint/test
