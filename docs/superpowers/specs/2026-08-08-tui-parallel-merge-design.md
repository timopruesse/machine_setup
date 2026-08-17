# TUI parallel awareness + merged log

Date: 2026-08-08  
Status: **superseded** by [ADR-0009](../adr/0009-runner-grid-details-pane.md)  
Repo: `timopruesse/machine_setup`  
Approach: **1** — ephemeral merge ring in the pure reducer

## Scope

Improve parallel-task UI/UX:

- **Awareness:** show how many tasks are running; stable accent colors on running names
- **Merged log:** multiplex live output into one tagged stream while ≥2 tasks are `Running`

Out of scope: engine / `TaskEvent` changes, plain mode, split panes, merge toggle, backfill, themes, mouse, auto-quit.

## Context

After the state-machine rewrite and elapsed-time polish, the TUI still shows a single selected task’s log. With parallel runs, `auto_select_running` jumped selection on every `TaskStarted`, and the log pane could only show one task at a time — poor situational awareness during parallel bursts.

## Goals

- Auto-enter a merged, task-tagged log when `running_count() >= 2`; leave when it drops to ≤1
- Live-only append (no backfill of earlier per-task history)
- List selection (`j`/`k`) highlights only; does not leave merge mode
- Soft auto-select: jump only when the currently selected task is not `Running`
- On merge exit: prefer a task that failed during the burst; else keep selection (soft-select the remaining runner if auto-select is on)
- Header/list cue: `N running`; palette colors shared by list accents and merge prefixes
- Keep pure `reduce(UiState, Message) -> (UiState, Effect)`; unit-test reducer behavior

## Non-goals

- Changing `TaskEvent` / runner / concurrency gate
- Plain / `--no-tui` multiplex
- Multi-pane layouts or on-demand merge toggle
- Backfilling merge from per-task logs
- Ratatui snapshot tests (reducer tests suffice)

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Approach | Ephemeral `merge_lines` ring owned by reducer |
| Enter / leave | Display merge iff `running_count() >= 2`; clear buffer on leave |
| Append rule | Live-only; append event lines when `running_before >= 2 \|\| running_after >= 2` so the leaving event’s final line is included |
| Selection while merged | List highlight only; log stays merged |
| Auto-select | Soft: only if selected task is not `Running` |
| Exit selection | First failure recorded during the burst, else current selection (+ soft-select last runner) |
| Awareness | `N running` (for `N ≥ 1`) + stable per-task palette colors |
| Caps | ~2000 lines for `merge_lines` (mirror `LOG_CAP`) |
| Engine boundary | Unchanged |

## Architecture

```
TaskEvent ─► reduce(UiState)
                │
                ├─ update per-task state (unchanged)
                ├─ if burst active (before│after ≥ 2 running): append tagged line(s) → merge_lines
                ├─ soft auto-select when selected leaves Running
                └─ when running drops ≤ 1 after a burst: exit selection, clear merge_lines
render:
  log pane → merge_lines if running ≥ 2, else selected task log
  list/header → “N running” + palette accents
```

### New / extended state

- `MergeLine { task_name, color_idx, text }`
- `merge_lines: Vec<MergeLine>` — ring-capped
- `merge_failed: Vec<usize>` — task indices that failed while a burst was active (order preserved)
- Per-task `color_idx: Option<usize>` — assigned lazily from a small fixed palette; stable for the run
- Helpers: `running_count()`, `in_merge_mode()`, `ensure_task_color()`, push helpers that update follow scroll for merge vs single log

### Widgets

- **Log (merged):** title ` Parallel · N `; yellow border; lines `name │ text` with colored prefix; follow/scroll operate on `merge_lines`
- **Task list:** running names tinted with palette color; title/bottom shows `N running` when `N ≥ 1`
- **Header:** light echo of running count in the gauge label when not done (optional clutter-safe)
- **Help:** optional one-line hint while merged (`j/k list`) if space allows

## Behavior detail

1. Apply engine event to the per-task log as today.
2. Record `running_before` / `running_after`.
3. If `running_before >= 2 || running_after >= 2`, append the same user-visible line(s) to `merge_lines` with task tag + color.
4. Soft auto-select on start/status changes when `auto_select_running` and selected is not `Running`.
5. Manual `j`/`k` still clears `auto_select_running`.
6. When `running_before >= 2 && running_after <= 1`: choose exit selection (failure-preferring), clear `merge_lines` + `merge_failed`, restore single-task log follow semantics.
7. Search/filter still filters the list only; merge log ignores the filter.

## Testing

Reducer unit tests:

- Enter merge on 2nd `TaskStarted`; no merge lines before that
- No thrash while selected remains running
- Soft follow when selected completes and another is still running
- Manual select disables auto-select
- Leave merge clears buffer; prefers failed-in-burst
- Merge cap drops oldest

## Ship shape

One cohesive change set on top of the current reducer/widgets (no engine PR).
