# Doctor catalog viewer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put `doctor` on the shared catalog Browse TUI (with summary banner) and a pretty plain fallback, then ship v2.6.1.

**Architecture:** Extend `CatalogState` with an optional banner; add `doctor_items` / `doctor_banner` adapters and `plain::render_doctor`; wire `run_doctor` with the same TUI gate as `list`. `--fix` and exit codes stay as today.

**Tech Stack:** Existing `src/tui/catalog/*`, `config::status::doctor`, ratatui, crossterm.

**Spec:** `docs/superpowers/specs/2026-08-12-doctor-catalog-viewer-design.md`

## Global Constraints

- TUI gate: `!cli.no_tui && stdout().is_terminal()`.
- Browse only; no select-mode; no new CLI flags.
- Orphans in banner + plain section only (not catalog rows).
- `--fix` after display; prune all orphans.
- Exit `1` on `report.has_errors()`.
- Do not change install run-TUI.
- Do not commit unless asked (controller may commit for release).

## File map

| File | Change |
| --- | --- |
| `src/tui/catalog/state.rs` | `banner: Option<Vec<String>>` |
| `src/tui/catalog/mod.rs` | `run_browse(items, banner: Option<Vec<String>>)` |
| `src/tui/catalog/view.rs` | Render banner above master–detail |
| `src/tui/catalog/adapt.rs` | `doctor_items`, `doctor_banner` + tests |
| `src/tui/catalog/plain.rs` | `render_doctor` / `print_doctor` + tests |
| `src/main.rs` | Wire `run_doctor` |
| `CHANGELOG.md` | Unreleased → then 2.6.1 on release |

---

### Task 1: Banner on catalog state + view + run_browse

- [ ] Add `pub banner: Option<Vec<String>>` to `CatalogState`; `new(items, mode)` sets `banner: None`; add `with_banner(mut self, banner: Option<Vec<String>>) -> Self`.
- [ ] Update `run_browse` to `run_browse(items, banner: Option<Vec<String>>)`; empty items + no banner → `No tasks defined.`; empty items + Some(banner) still opens TUI.
- [ ] Update list call site to `run_browse(items, None)`.
- [ ] In `view::render`, if banner present, allocate top lines (`banner.len().clamp(1, 6) + 2` for border) and render a bordered “ Doctor ” / “ Summary ” paragraph.
- [ ] `cargo test --lib catalog::` + `cargo check`.

### Task 2: doctor adapters

- [ ] `doctor_items(report: &DoctorReport<'_>) -> Vec<CatalogItem>`: map `report.rows` via existing row logic; for each task, append DetailSection `"Validation"` with matching issues (`[ERROR] msg` / `[WARN] msg`); add badges `error` / `warn` when present.
- [ ] `doctor_banner(report, fix: bool) -> Vec<String>`: e.g. validation line, orphan count, fix hint when orphans && !fix.
- [ ] Unit tests.
- [ ] Prefer refactoring `row_to_item` to accept optional issues slice to avoid duplication.

### Task 3: plain doctor + wire main

- [ ] `render_doctor(report-derived items, issues summary lines, orphans, color) -> String` or take structured args: items + validation lines + orphan names + fix hint.
- [ ] `print_doctor(...)`.
- [ ] Rewrite `run_doctor` to build report, then TUI or plain, then `--fix`, then exit 1 on errors. Pass `no_tui` from CLI.
- [ ] `make test && make lint`.

### Task 4: Changelog + v2.6.1

- [ ] CHANGELOG Unreleased bullets for doctor catalog.
- [ ] After merge/approval: bump to 2.6.1, tag, push (same as 2.6.0).
