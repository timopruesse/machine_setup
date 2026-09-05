# TUI SilkCircuit polish

Date: 2026-09-05  
Status: approved  
Repo: `timopruesse/machine_setup`  
Related: ADR-0009 (runner grid), `CONTEXT.md` (Details pane / Runner grid), catalog viewer specs (2026-08-12)

## Context

The run dashboard (`install` / `update` / `uninstall`) and catalog viewer (`list` / `doctor` / `--select`) already share a master–detail layout and a cyan/green/red vocabulary, but colors and help chrome are hardcoded per widget. The surfaces are functional; they are not a coherent SilkCircuit Neon identity, and several usability cues (quit semantics, follow mode, narrow terminals, in-TUI completion) are weak or inconsistent.

## Goals

- Shared **SilkCircuit Neon** theme tokens for run + catalog interactive TUIs.
- Same panel layouts; visual hierarchy and targeted UX polish only.
- Discoverable help (including Esc quit when idle/done; `q`/Ctrl+C cancel while running).
- Explicit follow-mode cue when auto-follow is off.
- Clear selection / band emphasis via accent + focused border (no multi-pane focus model).
- In-TUI completion strip when the run finishes (success or failure).
- Narrow-terminal collapse: drop Details below a width threshold; keep Header + Tasks + Help + quit/nav.
- Honor non-empty `NO_COLOR` before RGB; fall back to named ANSI / bold hierarchy.

## Non-goals

- New panels, mouse support, command palette, or multi-pane focus rings.
- Pluggable theme engine / user theme config.
- Changing runner-grid max of 4 bands.
- Engine / `TaskEvent` contract changes.
- Plain-mode redesign (text prefixes may stay; no Neon chrome requirement).
- Full ratatui snapshot suite (still deferred per ADR-0009).

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Scope | Run dashboard + catalog; shared visual language |
| Approach | Theme module + targeted UX; panels unchanged |
| Look | SilkCircuit Neon (electric purple + neon cyan on dark chrome) |
| Borders | Rounded (`╭╮╰╯`); focused/selected uses `border_focus` |
| Focus model | Implicit selection only (task / band); no Tab-between-panes |
| Narrow width | Collapse Details when main width &lt; 68; restore when wide; preserve selection/scroll |
| Completion | Header gauge label strip `N ok · M failed · elapsed`; keep post-exit `print_summary` |
| `NO_COLOR` | Checked before RGB; mono/ANSI semantic slots |
| Theme API | Resolve once at TUI start; read-only in render |

## Visual system

Introduce `src/tui/theme.rs` as the single source of semantic slots. Widgets stop hardcoding `Color::Cyan` / `Green` / etc.

| Slot | Role | Neon hex |
| --- | --- | --- |
| `accent` | keys, focus chrome, primary brand | `#e135ff` |
| `accent_alt` | interactions, secondary focus | `#80ffea` |
| `success` / `error` / `warning` | status | `#50fa7b` / `#ff6363` / `#f1fa8c` |
| `info` | command starts, info lines | `#80ffea` |
| `muted` | help actions, pending, idle chrome | `#82879f` |
| `text` | primary labels | `#f8f8f2` |
| `border` / `border_focus` | idle vs focused panel | `#3c3c50` / `#e135ff` |
| `gauge_bg` | header track | `#37324b` |
| task palette (8) | stable per-task accents | purple, cyan, coral, green, yellow, magenta, deep purple, pink |

`Theme::neon()` vs `Theme::mono()` selected at interactive TUI entry after `NO_COLOR` check. Catalog and run both receive `&Theme` (Copy/Clone if the struct is small).

Shared chrome helpers in `src/tui/widgets/chrome.rs`: rounded `Block` builders and `key_hint(key, action)` used by run help bar and catalog footer.

## Layout & UX

### Panels (unchanged skeletons)

- **Run:** Header (3 rows) → horizontal Tasks | Details → Help (1 row).
- **Catalog:** optional summary banner → Tasks | Detail → Help (1 row).

### Discoverability

- Help bar lists only currently valid actions.
- When running: advertise `q`/Ctrl+C as cancel+quit; when idle/done with no active filter: advertise Esc as quit without cancel (match `Effect` semantics).
- When `!log_follow`: show follow cue (e.g. `End follow`) consistently and themed.
- Deduplicate key-hint rendering across run and catalog.

### Focus & selection

- Selected list row and selected details band share the task palette accent.
- Focused panel title / border uses `border_focus`; idle panels use `border`.
- Keep `parallel_burst` selection policy; selection remains identity-stable across filter and burst enter/leave (fix index-only gaps if tests reveal them).

### Completion / failure

- On run `done`: header gauge/border uses `success` or `error`.
- Header gauge label includes the completion strip: `N ok · M failed · elapsed` (details pane keeps normal task title; status still via border color).
- On `AllDone` with failures: keep auto-select first failed; failed row/band uses `error` + glyph.

### Narrow terminals

- Pure layout function of `Rect`. Constant `DETAILS_MIN_WIDTH: u16 = 68`: when the main content area width is below that, collapse Details to full-width Tasks only.
- Height floor: if total rows < 8, draw Help + a one-line “terminal too small” message; never treat zero-sized transient chunks as fatal.
- Collapsed Details: state (`selected`, `log_scroll`, follow) preserved; restored when width returns.

## Architecture

```
events → reduce → UiState / CatalogState → render(theme, state, area)
```

| Piece | Change |
| --- | --- |
| `src/tui/theme.rs` | New: slots, neon/mono, palette |
| `widgets/*`, `details/render.rs`, `log_display.rs` | Consume theme; rounded borders |
| `format.rs` | Task palette from theme (or theme owns palette) |
| `mod.rs` layout | Narrow collapse constraints |
| `catalog/view.rs` | Same tokens + shared key hints |
| Engine / plain | Untouched (plain optional muted ANSI only if already present) |

Theme is immutable for the session. Layout collapse is view-only.

## Error / edge cases

- Zero-area chunks: skip draw for that region; input loop continues.
- Late engine events after `done`: existing no-op behavior.
- Help bar copy must match `Effect::Quit` vs `Effect::CancelAndQuit` exactly.
- Resize storms: recompute layout each frame; clamp scroll after filter/viewport shrink (existing patterns).

## Testing

- Unit: theme under `NO_COLOR`; help-bar hint sets for running / searching / done / burst; narrow splitter constraints.
- Keep existing reducer / `parallel_burst` tests green; add selection-stability coverage if gaps appear.
- Optional cheap `TestBackend` smoke for header+help if low cost; no full snapshot suite.

## Docs / follow-ups

- Spec path: this file.
- Sync README keybinding / parallel UI note if still stale vs ADR-0009.
- `CONTEXT.md` only if new canonical terms appear (unlikely; reuse Details pane / Runner grid).

## Success criteria

- Interactive run + catalog use SilkCircuit Neon tokens (or mono under `NO_COLOR`).
- Panel layouts unchanged; narrow collapse and completion strip work.
- Help bar reflects real quit/cancel/follow/burst actions.
- `make check && make test && make lint` green.
- No engine contract changes.
