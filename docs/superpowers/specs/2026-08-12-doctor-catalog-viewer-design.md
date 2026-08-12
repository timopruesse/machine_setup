# Doctor catalog viewer

Date: 2026-08-12  
Status: approved  
Repo: `timopruesse/machine_setup`  
Extends: `docs/superpowers/specs/2026-08-12-catalog-viewer-list-select-design.md`  
Ship: doctor TUI + pretty plain; then **v2.6.1**

## Context

`list` and `-s` already use the shared catalog viewer (v2.6.0). `doctor` still prints the old flat dump. The parent design reserved Browse + richer items + optional summary banner for doctor.

Out of scope (explicitly declined): select-mode orphan picking, new CLI flags, manual Homebrew/tap work beyond the normal `v*` tag pipeline.

## Goals

- `doctor` uses catalog Browse TUI when `!no_tui && stdout().is_terminal()`.
- Pretty plain fallback (aligned columns + Validation + Orphans), `NO_COLOR` / non-TTY respected.
- Summary banner: validation status/counts, orphan count, `--fix` hint when orphans exist and `--fix` was not passed.
- Task detail includes that task’s validation issues (in addition to list-style meta/history/commands).
- `--fix` semantics unchanged: after TUI quit or plain print, prune all orphans, save History, print removed names.
- Exit code `1` when `report.has_errors()` (unchanged).

## Non-goals

- Multi-select / selective orphan prune in the TUI.
- New flags (`--json`, TUI confirm for `--fix`, etc.).
- Changing validation / doctor report data model beyond display adapters.
- Touching install run-TUI.

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Layout | Master–detail Browse + top summary banner |
| Orphans in TUI | Banner (and plain section); not separate catalog rows |
| Task issues | In selected task detail as a “Validation” section |
| `--fix` | After display; prune all orphans (existing behavior) |
| TUI gate | Same as `list` |
| Release | v2.6.1 after merge |

## Architecture

- `adapt::doctor_items(report) -> Vec<CatalogItem>` — like `list_items`, plus per-task Validation detail when issues exist for that name; status may use `CatalogStatus` Issue-like cue via badge or existing status + “error”/“warn” badges.
- `CatalogState` gains `pub banner: Option<Vec<String>>` (or small `CatalogBanner { lines: Vec<String> }`).
- `run_browse` gains an optional banner parameter (or `run_browse_with_banner`) without breaking list callers (`None`).
- `view::render` — if banner present, vertical chunk `Length(banner_lines.clamp(1..4)+2)` above the list/detail split.
- `plain::print_doctor` / `render_doctor` — task table (reuse list row rendering) + Validation block + Orphans block.

Wire in `run_doctor` in `main.rs`.

## Testing

- Adapter tests: issue attached to matching task detail; orphan-only tasks not invented as rows.
- Plain doctor renderer tests (color off): contains Validation / orphans headers.
- Banner lines helper unit-tested (counts / valid / fix hint).
- Existing catalog reduce tests unchanged.

## Success criteria

- TTY: `machine_setup doctor` opens catalog Browse with banner.
- `--no-tui`: pretty plain with three sections.
- `--fix` still prunes and reports; exit 1 on validation errors.
- `make check && make test && make lint` green.
- Tag `v2.6.1` after changelog bump.
