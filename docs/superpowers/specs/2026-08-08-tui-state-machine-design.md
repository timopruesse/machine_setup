# TUI thin rewrite: state machine + UX hardening

Date: 2026-08-08  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **2** — thin rewrite around a pure `UiState` / `Message` reducer  
Ship: one cohesive PR (bugs + polish + internals)

## Context

The interactive UI lives under `src/tui/`:

- `mod.rs` — terminal setup, blocking poll loop, layout
- `app.rs` — mutable `App` that both handles `TaskEvent`s and keyboard
- `widgets/*` — header, task list, log, help bar
- `plain.rs` — CI / `--no-tui` line printer

It already works for install/update/uninstall with search, nested-task indent, and
a progress gauge. Gaps:

1. **UX bugs** — log auto-scroll fights manual scroll; after `/`+Enter the filter
   sticks with no safe Esc clear (Esc quits); help bar omits real bindings
   (`j`/`k`, `Home`/`End`).
2. **Polish** — static `>>` for running tasks; no jump-to-first-failure when done;
   plain skip lines ignore nesting depth.
3. **Internals** — blocking `crossterm::event::poll` inside an async loop; no
   unit tests on UI state; unbounded `log_lines`; dead `TaskStatus` helpers.

Engine `TaskEvent` / sink stay as-is. Presentation is rewritten for testability
and the UX fixes above.

## Goals

- Pure reducer: `reduce(UiState, Message) -> (UiState, Effect)` with no I/O.
- Fix log follow, filter Esc semantics, and help accuracy.
- Spinner via tick; on `AllDone` with failures, select first failed task.
- Cap per-task log buffers; async-friendly input loop.
- Unit-test reducer behavior (events, search/filter, follow, failure jump, cap).
- Keep the same visual layout (header / list / log / help).

## Non-goals

- Changing `TaskEvent` schema or engine runner.
- Redesigning plain output beyond depth indent on skips.
- Mouse support, themes, configurable keybinds.
- Ratatui snapshot / golden tests in this PR.
- Auto-exit when done (user still reviews with `q`).

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Approach | Thin rewrite: reducer + widgets on `&UiState` |
| Ship shape | One PR covering A (bugs) + B (polish) + C (internals) |
| Esc vs quit | `q` always cancels/quits; Esc clears active filter if any, else quits |
| Log follow | Default on; PgUp/Home disable; End / stick-to-end only when follow on |
| Log scroll | Logical line scroll with wrap kept; follow = stick to end after append |
| Log cap | Ring buffer ~2000 lines per task; drop oldest |
| Spinner | `Tick` message ~100–200ms; animate running rows |
| Done + failures | Jump selection to first failed task; turn follow off |
| Progress | `completed / tasks.len()` (honest as nested tasks appear) |
| Plain mode | Indent skips by `depth`; not on the reducer path |
| Engine boundary | Unchanged `TaskEvent` / `ChannelSink` |

## Architecture

```
TaskEvent (engine) ─┐
Key / tick (UI)    ─┼─► Message ─► reduce(UiState) ─► UiState
                    ┘                    │
                              render(UiState) / effects (cancel, quit)
```

- **`UiState`** — sole source of truth (replaces `App`).
- **`Message`** — `Engine(TaskEvent)` \| `Input(...)` \| `Tick`.
- **`Effect`** — `None` \| `Cancel` (fire `CancellationToken`) \| `Quit`
  (leave the UI loop; may combine with Cancel on `q` / Ctrl+C).
- **`run` loop** — `tokio::select!` on engine channel, key mpsc (crossterm read
  on a blocking helper thread), and tick interval; drain engine events; reduce;
  draw once; apply effects.
- **Widgets** — take `&UiState` only; same layout constraints as today.
- **`plain`** — remains a direct `TaskEvent` consumer.

Panic hook continues to restore the terminal (raw mode + leave alternate screen).

## Module layout

```
src/tui/
  mod.rs          // run(), terminal setup/restore, print_summary
  state.rs        // UiState, TaskState, TaskStatus
  message.rs      // Message, Effect, Input
  reduce.rs       // reduce(state, msg) -> (UiState, Effect)
  loop.rs         // async select: events + keys + tick
  plain.rs        // CI consumer
  widgets/        // header, task_list, log_view, help_bar
```

`app.rs` is removed after the split (or reduced to a thin re-export only if
needed during migration — prefer delete).

## State model

### `UiState`

- `tasks: Vec<TaskState>` — name, status, depth, `command_count`,
  `current_command`, capped `log_lines`
- Selection: `selected`, `filtered_indices`, `auto_select_running`
- Search: `search_mode`, `search_query` (non-empty query keeps filter after
  Enter; clearing is via Esc — see keys)
- Log: `log_scroll`, `log_follow` (default `true`)
- `tick: u64` for spinner frame
- Counts: `succeeded`, `failed`, `skipped`; `done: bool`
- `mode: Mode` (for header label)

### Keys (normal)

| Key | Behavior |
| --- | --- |
| `q` | Cancel + quit |
| Esc | If filter active → clear filter; else quit |
| `/` | Enter search mode |
| `j` / Down | Next filtered task; disable auto-select |
| `k` / Up | Prev filtered task; disable auto-select |
| PgUp / PgDn | Scroll log; PgUp disables follow |
| Home | Log top; disable follow |
| End | Log bottom; enable follow |
| Ctrl+C | Cancel + quit |

### Keys (search mode)

| Key | Behavior |
| --- | --- |
| Esc | Cancel search; clear query + filter |
| Enter | Exit search mode; keep filter if query non-empty |
| chars / Backspace | Edit query; refresh `filtered_indices` |
| Up / Down | Navigate filtered list |

### Help bar

Reflect real bindings; when `!log_follow`, show a follow hint (e.g. `End follow`).
When filter active outside search mode, show Esc clears filter.

## Effects & loop

1. Enable raw mode + alternate screen; install restore-on-panic hook.
2. Spawn key reader → `mpsc` of key events.
3. `select!` on engine recv, key recv, tick.
4. On wake: drain all pending `TaskEvent`s into `Message::Engine`, reduce each;
   then handle key/tick; `terminal.draw`; match `Effect`.
5. Restore terminal; print summary to stdout (same shape as today).

## Testing

Unit tests in `reduce.rs` (or `reduce/tests`):

- Engine lifecycle updates status counts and log lines
- Search filter + Enter retain + Esc clear without requiring quit
- Manual scroll disables follow; output does not snap when follow off
- End re-enables follow
- `AllDone` with failures selects first failed task
- Log cap drops oldest lines
- Selection stays within filtered set

No widget/golden tests in this PR.

## Implementation order (for the plan)

1. Introduce `state` / `message` / `reduce` beside existing `App`; port event
   handling into `reduce`; add tests for A/B behaviors.
2. Point widgets at `UiState`; delete `App`.
3. Replace poll loop with `loop.rs` async select + tick spinner.
4. Plain depth indent; help bar; wire `run` in `mod.rs`.
5. `make lint` + `make test`.

## Risks

- Async key reading + draw races: mitigate by single-threaded reduce/draw on
  the TUI task only.
- Wrap + scroll mismatch: accept logical-line scroll; follow always targets
  last logical line.
- Larger diff than a surgical patch: offset by reducer tests and unchanged
  layout/engine API.
