# Runner grid Details pane replaces parallel merge stream

Supersedes the locked decisions in
`docs/superpowers/specs/2026-08-08-tui-parallel-merge-design.md` (flat
interleaved `merge_lines` stream, list selection ignored in the details pane).

## Problem

While ≥2 Tasks were `Running`, the TUI multiplexed output into one tagged,
arrival-ordered stream. High-volume Tasks drowned quieter ones; `j/k` only
moved list highlight while the details pane kept showing the merge — misleading
navigation and poor situational awareness during parallel bursts.

## Decision

Replace the merge stream with a **Details pane** module and **Runner grid**
layout:

| Topic | Choice |
| --- | --- |
| Parallel burst UI | **Runner grid**: up to four fixed bands (one per running Task), each showing command progress and a scrolling tail from that Task's `log_lines` |
| Overflow | Title bar shows `(+N more)` when more than four Tasks are running |
| Band selection | `j/k` selects the focused band; scroll (`PgUp/PgDn`, `Home/End`) applies to the selected band only |
| Full scrollback | `Enter` toggles **expanded** mode: selected Task's full log with an “N others running” cue; `Enter` again collapses back to the grid |
| Burst lifecycle | Enter grid when `running_count >= 2`; leave when `running_count <= 1` (same trigger as the old merge mode) |
| Module shape | **`parallel_burst`**: burst selection, failure tracking, scroll/follow — not in `reduce` |
| | **`details`**: view resolution (`SingleTask` / `RunnerGrid` / `ExpandedTask`) + ratatui render adapter |
| Merge stream | **Removed** — `merge_lines` / `MergeLine` deleted; per-task logs are the single source of truth |
| Command progress | `CommandStarted` / `CommandCompleted` / `CommandFailed` carry `command_index` and `command_total` (1-based); Runner emits them for sequential and parallel command execution; plain mode prefixes `(i/n)` |

The **Task event sink** seam (ADR-0005) is unchanged — new fields on existing
event variants only.

## Rejected (from 2026-08-08 spec, kept rejected)

- Flat interleaved merge ring
- List selection that does not affect the details pane during a burst
- Multi-pane as an optional toggle (grid is the default burst presentation, not a mode switch)

## Reopened and accepted (previously non-goals in 2026-08-08)

- Per-runner spatial layout (Runner grid bands)
- `Enter` drill-down to full Task log during a burst

## Deferred

- Ratatui snapshot tests (reducer + `details::resolve` unit tests remain the test surface)
- Plain / `--no-tui` grid layout (plain keeps tagged line prefixes; no multiplex layout change)
