# Shared catalog viewer for `list` and `-s`

Date: 2026-08-12  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **3** — shared catalog viewer framework  
Ship: framework + `list` (TUI + pretty plain) + migrate `-s` onto the viewer; `doctor` later

## Context

Today:

- `list` always prints a flat dump via `print_task_list` in `main.rs` (never uses ratatui).
- Install/update/uninstall use the run TUI when `!no_tui && stdout.is_terminal()`, else `tui::plain`.
- `-s` / `--select` uses `dialoguer::MultiSelect` with task names only (no status/detail).
- `config::status::rows` already joins Config + History for `list` / `doctor`.
- The run TUI (`UiState` / `reduce` / widgets) is built around live execution (spinners, merge logs) and must not absorb browse/select concerns.

We want `list` to feel first-class: TUI by default when available, prettier plain fallback, and a reusable shell that `-s` (now) and `doctor` (later) can share.

## Goals

- Shared catalog viewer under `src/tui/catalog/` with master–detail layout, `/` filter, and mode-specific keys.
- `list`: TUI browse by default when TTY and not `--no-tui`; pretty colored plain otherwise.
- `-s`: same viewer in select mode when TUI available; keep `dialoguer` when TUI is off but stdin is a TTY.
- Adapters map `status::rows` (and select) into a neutral `CatalogItem` model.
- Pure reducer for catalog state (testable without a terminal).
- Reserve extension points so `doctor` can plug in later without reshaping the framework.

## Non-goals

- Doctor TUI or doctor plain redesign in this change.
- Changing engine, History semantics, or run-TUI behavior.
- Mouse support, themes, configurable keybinds.
- Machine-parseable / JSON list output (pretty human plain only).
- Replacing dialoguer for non-catalog prompts (uninstall dep confirm, etc.).

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Approach | Shared catalog framework (not bolt-on to run TUI; not list-only module) |
| Ship scope | Framework + `list` + `-s`; doctor later |
| TUI gate | `!cli.no_tui && stdout().is_terminal()` |
| List TUI | Browse: navigate + filter + quit; detail pane always shows focused task |
| Select TUI | Multi-check with Space; `a` = select all **visible** (filtered); Enter confirms; Esc/`q` aborts |
| Select empty confirm | Confirm with zero checks → abort (same spirit as “no tasks selected”) |
| Select no-TUI | Keep `dialoguer::MultiSelect` when stdin is a TTY; otherwise fail clearly |
| `--no-tui` + list | Pretty plain path |
| `--no-tui` + `-s` | dialoguer (not pretty dump) |
| Plain list style | Colored aligned columns + unicode glyphs; respect non-TTY and `NO_COLOR` |
| Layout | Master–detail (list + detail pane), not accordion |
| Filter | `/` search included (parity with install TUI) |
| Esc | Clears active filter if any, else quits/aborts (browse quit / select abort) |
| `q` | Always quit (browse) / abort (select) |
| Color deps | `crossterm::style` (already depended); no new color crate |
| Run TUI | Untouched except optional shared terminal bootstrap/restore if extraction is trivial |

## Architecture

```text
src/tui/catalog/
  mod.rs      — run_browse / run_select; re-exports
  model.rs    — CatalogItem, CatalogStatus, DetailSection, CatalogMode
  state.rs    — CatalogState (items, filtered indices, cursor, checked set, search)
  reduce.rs   — reduce(state, msg) -> (state, CatalogEffect)
  message.rs  — Input / Message / CatalogEffect
  view.rs     — ratatui: list + detail + help + search line
  plain.rs    — pretty list printer
  adapt.rs    — list_items / select_items from AppConfig + History
```

Public call sites in `main.rs`:

- `Command::List` → build items → `catalog::run_browse` or `catalog::plain::print_list`
- `-s` path → `catalog::run_select` or existing dialoguer fallback

Install run TUI stays in `src/tui/{state,reduce,event_loop,widgets}` as today.

### Data model

```rust
CatalogStatus { Installed, NotInstalled, SkippedOs, Neutral }

CatalogItem {
  id: String,
  title: String,
  status: CatalogStatus,
  badges: Vec<String>,           // e.g. "parallel", OS label when useful
  detail: Vec<DetailSection>,    // { title, lines }
}

CatalogMode { Browse, Select }

CatalogEffect { None, Quit, Abort, Confirm(Vec<String> /* ids */) }
```

Adapters:

- `list_items(config, history)` from `status::rows` — status glyphs, OS badges, timestamps + commands in detail.
- `select_items(config, history)` — same status cues so selection is informed; ids are task names.

Empty item list: list prints a one-liner and exits 0; select aborts.

### Layout (TUI)

```text
┌─ Tasks (N) ─────────────────────────────┐
│ > [✓] dotfiles          parallel        │
│   [·] neovim                            │
│   [–] windows-only      os skip         │
├─ Detail ────────────────────────────────┤
│ OS: macos                               │
│ Installed: yes · 2026-08-01 12:00 UTC   │
│ Updated: -                              │
│ Commands:                               │
│   - symlink …                           │
├─────────────────────────────────────────┤
│ q quit · j/k navigate · space toggle · /│
└─────────────────────────────────────────┘
```

Browse help omits Space/`a`/Enter-confirm. Select help includes them. Search mode help matches install TUI patterns (Esc cancel, Enter apply).

### Plain list

- Header: `Tasks` with counts (total / installed).
- Columns: glyph · name · OS · installed_at · updated_at · badges.
- Indented command lines under each task (existing `Display` of commands).
- Glyphs: `✓` installed, `·` not installed, `–` OS skipped on this host.
- Color on when stdout is a TTY and `NO_COLOR` is unset; otherwise plain text.
- Truncate long names with `…`; keep commands on their own lines.

### Select fallback matrix

| Condition | Behavior |
| --- | --- |
| TUI available | Catalog select mode |
| `--no-tui` or non-TTY stdout, stdin is TTY | `dialoguer::MultiSelect` |
| No usable interactive input | Error: cannot select interactively |

### Doctor (later — interface only)

- Reuse `Browse` with richer `CatalogItem` detail and/or an optional summary banner (validation errors, orphans) supplied by a future `doctor_items` adapter.
- No doctor UI code in this change; avoid APIs that assume list-only fields.

## Error handling

- Terminal setup/teardown mirrors run TUI (restore on exit and panic hook).
- Adapter/history load failures: same as today (`History::load(...).unwrap_or_default()` for list).
- Select abort → `Aborted.` / no tasks run (existing main flow).

## Testing

- Unit-test `reduce`: navigate, filter apply/clear, Space toggle, `a` select-all-visible, Confirm/Abort/Quit, Esc semantics.
- Unit-test adapters: installed / not installed / os-skip badges and detail sections.
- Plain formatter tests with color forced off (stable strings).
- No ratatui golden/snapshot tests required in this PR.

## Migration notes

- Remove `print_task_list` body from `main.rs` (delegate to catalog).
- `select_tasks` becomes: try catalog select when TUI gate passes, else dialoguer.
- Uninstall dependency MultiSelect / Confirm stay on dialoguer for now.

## Success criteria

- Interactive TTY: `machine_setup list` opens browseable master–detail TUI; `q` exits.
- `machine_setup list --no-tui` (or piped stdout) shows prettier columnar plain output without color when appropriate.
- `machine_setup install -s` (TTY, TUI on) uses catalog multi-select with status/detail; Enter runs chosen tasks.
- `--no-tui` + `-s` still uses dialoguer.
- `make check` / `make test` / `make lint` green.
- Run TUI for install unchanged in behavior.
