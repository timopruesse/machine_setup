# TUI state-machine rewrite — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the interactive TUI around a pure `reduce(UiState, Message) -> (UiState, Effect)` loop so UX bugs (log follow, Esc/filter), polish (spinner, jump-to-failure), and internals (async keys, log cap, tests) land in one cohesive change.

**Architecture:** Split `app.rs` into `state` / `message` / `reduce`; drive the UI from an async `select!` loop (engine events + key channel + tick). Widgets render `&UiState` only. Engine `TaskEvent` unchanged.

**Tech Stack:** Rust, tokio, ratatui 0.30, crossterm 0.29, existing `TaskEvent` / `Mode`.

**Spec:** `docs/superpowers/specs/2026-08-08-tui-state-machine-design.md`

## Global Constraints

- Do not change `TaskEvent` schema or engine runner/sink.
- Same layout: header (3) / main (list 30% + log 70%) / help (1).
- `q` / Ctrl+C → Cancel + Quit; Esc clears active filter if any, else Quit.
- Log follow default on; PgUp/Home disable; End enables; appends stick only when follow on.
- Per-task log cap: 2000 lines (drop oldest).
- No ratatui golden tests; no mouse/themes/keybind config.
- Do not commit unless the user explicitly asks (leave a clean diff for `/land` or committer).

## File map

| File | Responsibility |
| --- | --- |
| `src/tui/state.rs` | `UiState`, `TaskState`, `TaskStatus`, `LOG_CAP`, constructors/helpers |
| `src/tui/message.rs` | `Message`, `Input`, `Effect` |
| `src/tui/reduce.rs` | `reduce`, unit tests |
| `src/tui/loop_ui.rs` | async event loop (named to avoid `loop` keyword) |
| `src/tui/mod.rs` | `run`, terminal setup/restore, `print_summary`, layout `render` |
| `src/tui/plain.rs` | indent skips by depth |
| `src/tui/widgets/*` | render from `&UiState`; spinner + help hints |
| `src/tui/app.rs` | **delete** after migration |
| `CHANGELOG.md` | `[Unreleased]` Changed/Fixed bullets |

**Note:** Spec says `loop.rs`; Rust reserves `loop` as a keyword for modules in some contexts — use `loop_ui.rs` with `mod loop_ui` (or `event_loop.rs`). Prefer `event_loop.rs` for clarity.

---

### Task 1: State + message types + failing reducer tests

**Files:**
- Create: `src/tui/state.rs`
- Create: `src/tui/message.rs`
- Create: `src/tui/reduce.rs` (tests only first is fine; stub `reduce` that panics or returns noop until Task 2)
- Modify: `src/tui/mod.rs` — `mod state; mod message; mod reduce;`

**Interfaces:**
- Produces:
  - `pub const LOG_CAP: usize = 2000;`
  - `TaskStatus { Pending, Running, Completed, Failed(String), Skipped(String) }`
  - `TaskState { name, status, log_lines, command_count, current_command, depth }`
  - `UiState { tasks, selected, mode, log_scroll, log_follow, done, succeeded, failed, skipped, auto_select_running, search_mode, search_query, filtered_indices, tick }`
  - `UiState::new(task_names: Vec<String>, mode: Mode) -> Self`
  - `Message::{ Engine(TaskEvent), Input(Input), Tick }`
  - `Input::{ Quit, CancelQuit, ClearFilterOrQuit, EnterSearch, ConfirmSearch, ExitSearch, SearchChar(char), SearchBackspace, SelectNext, SelectPrev, LogPageUp, LogPageDown, LogHome, LogEnd }`
  - `Effect::{ None, Cancel, Quit, CancelQuit }` — or `Cancel` + separate quit flag; prefer `Effect { cancel: bool, quit: bool }` **or** enum `None | Cancel | Quit | CancelAndQuit`. Spec: Cancel fires token; Quit leaves loop; `q` does both → use `CancelAndQuit` and `Quit` and `None`.
  - `pub fn reduce(state: UiState, msg: Message) -> (UiState, Effect)`

- [ ] Add `state.rs` / `message.rs` with the types above (mirror fields from current `App`, plus `log_follow: true`, `tick: 0`).
- [ ] Add `reduce.rs` with a stub `reduce` and `#[cfg(test)]` module containing failing tests for:
  1. `engine_task_started_marks_running_and_auto_selects`
  2. `search_enter_keeps_filter_esc_clears_without_quit` — `/` via EnterSearch, chars, ConfirmSearch, then ClearFilterOrQuit → filter cleared, `Effect::None` (not Quit)
  3. `log_page_up_disables_follow_output_does_not_snap`
  4. `log_end_reenables_follow`
  5. `all_done_with_failures_selects_first_failed`
  6. `log_cap_drops_oldest`
  7. `selection_stays_within_filtered_set`
- [ ] Wire modules in `mod.rs` (keep existing `app` for now).
- [ ] Run `cargo test reduce:: --lib` — tests fail until Task 2.

### Task 2: Implement `reduce`

**Files:**
- Modify: `src/tui/reduce.rs`

**Interfaces:**
- Consumes: Task 1 types
- Produces: fully working `reduce`

- [ ] Port `App::handle_event` logic into `Message::Engine` arm; push logs via helper that enforces `LOG_CAP`.
- [ ] On log append: if `log_follow`, set `log_scroll` to end (`log_lines.len().saturating_sub(1)`).
- [ ] On `AllDone`: set `done`; if any `Failed`, set `selected` to first failed index, `log_follow = false`, `log_scroll = 0` (or end of that task’s log without follow — prefer scroll 0 so failure context at top is ok; or End of log — prefer last lines of failed task with follow off so user sees the error: set scroll to end, follow false).
- [ ] Input arms per spec key table; `ClearFilterOrQuit`: if `!search_query.is_empty() || search_mode` clear search + full filter refresh → `Effect::None`; else `Effect::Quit`.
- [ ] `CancelAndQuit` / `Quit` return matching effects; `Tick` increments `tick`.
- [ ] `SelectNext`/`SelectPrev` set `auto_select_running = false`, navigate `filtered_indices`, reset `log_scroll`, leave `log_follow` as-is (or keep follow on when switching tasks — prefer reset scroll to 0 and keep follow true when switching so new task tails; when switching, set `log_follow = true` and scroll to end).
- [ ] Make all Task 1 tests pass: `cargo test reduce:: --lib`.

### Task 3: Widgets + delete `App`

**Files:**
- Modify: `src/tui/widgets/header.rs`, `task_list.rs`, `log_view.rs`, `help_bar.rs`
- Modify: `src/tui/mod.rs` — render uses `UiState`
- Delete: `src/tui/app.rs`

- [ ] Change widget imports from `crate::tui::app::{App, TaskStatus}` to `crate::tui::state::{UiState, TaskStatus}`.
- [ ] Task list: running symbol from spinner frame `["|", "/", "-", "\\"][(tick % 4) as usize]` instead of static `>>`.
- [ ] Help bar: show `j/k`, `Home/End`; if `!log_follow` show `End follow`; if filter active and not search_mode show `Esc clear`; if search_mode keep Esc/Enter hints.
- [ ] Log view: use `log_scroll` / wrap as today; respect follow (scroll already in state).
- [ ] Update `mod.rs` `render` / `print_summary` to `UiState`; remove `mod app`.
- [ ] Temporarily keep old poll loop compiling by constructing `UiState` and mapping keys → `Input` → `reduce` (or skip to Task 4 if doing both together). Prefer Task 4 in the same sitting if compile requires it.

### Task 4: Async event loop

**Files:**
- Create: `src/tui/event_loop.rs`
- Modify: `src/tui/mod.rs`

- [ ] Implement `pub async fn run_loop(terminal, state, event_rx, cancel) -> Result<()>`:
  - Spawn blocking task: loop `crossterm::event::read()` → send `KeyEvent` on `mpsc::unbounded_channel` until cancel/drop.
  - `tokio::time::interval(Duration::from_millis(150))` for ticks.
  - `tokio::select!` biased or fair: `event_rx.recv()`, `key_rx.recv()`, `tick.tick()`, `cancel.cancelled()`.
  - On engine: drain with `try_recv` after first recv; `reduce` each `Message::Engine`.
  - Map keys to `Input` (search vs normal) then `reduce`.
  - Draw after batch; on `Effect` with cancel → `cancel.cancel()`; on quit → break.
- [ ] `tui::run` calls `event_loop::run_loop` instead of inline poll.
- [ ] Panic hook + restore + `print_summary` unchanged in behavior.

### Task 5: Plain indent + CHANGELOG + verify

**Files:**
- Modify: `src/tui/plain.rs`
- Modify: `CHANGELOG.md`

- [ ] `TaskSkipped`: print `"  ".repeat(depth)` before `-- Skipped` (add `depth` from event — **event already has no depth on skip**). Check `TaskEvent::TaskSkipped` — if no depth field, keep as-is OR use 0. Spec says indent skips by depth; if event lacks depth, either skip this polish or extend event. **Do not change TaskEvent** per constraints — then plain indent only where depth exists (`TaskStarted` already indents). For skips: leave unchanged if no depth on event, note in CHANGELOG only what shipped. Re-read event: `TaskSkipped { task_name, reason }` — no depth. Spec non-goal conflict: "Plain mode: Indent skips by depth" vs "Do not change TaskEvent". **Resolution:** indent using best-effort 0, or drop plain skip indent from this PR. Prefer drop (YAGNI vs schema change).
- [ ] CHANGELOG `[Unreleased]`: Changed — TUI reducer/async loop; Fixed — log follow, Esc clears filter, etc.
- [ ] Run `make lint` and `make test`; fix issues.

### Task 6: Stop for human commit/PR

- [ ] Leave working tree ready; do not commit unless asked.
- [ ] Summarize behavior changes for the user.
