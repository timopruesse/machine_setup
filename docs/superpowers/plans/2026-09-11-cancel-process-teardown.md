# Cancel Process Teardown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On cancel, tear down OS process groups (TERM→grace→KILL), poll File ops between files/chunks, and surface distinct Cancelling/Cancelled UX — with a before/after Criterion soft gate.

**Architecture:** Centralize spawn group setup + cancel-aware wait in `utils/process.rs`. Runner emits `RunCancelling` / `TaskCancelled` and tallies `AllDone.cancelled`. Tree materialization accepts an optional cancel token and returns `Error::Aborted` between files/chunks. TUI/plain render the new statuses.

**Tech Stack:** Rust, Tokio `process` + `CancellationToken` (`tokio_util`), `libc` (Unix signals/process groups), existing Command bench (Criterion).

**Spec:** `docs/superpowers/specs/2026-09-11-cancel-process-teardown-design.md`

## Global Constraints

- Signal policy: **SIGTERM → ~2s grace → SIGKILL** (Unix); Windows best-effort process-tree terminate with the same grace idea.
- Cooperative cancel inside `stream_and_wait` owns kill — do not JoinSet-abort a task before teardown runs.
- File ops: poll between files/chunks only; current single-file op may finish.
- `TaskCancelled` ≠ `TaskFailed`; `TaskStatus::Cancelled` is done, not failed.
- History: only `TaskCompleted` updates install record.
- Exit non-zero if `cancelled > 0` or `failed > 0`.
- Soft bench gate: investigate if `tree_install_direct/1k` or `runner_smoke/*` regress **>~5%**.
- Do **not** `git commit` in worker tasks — leave a clean diff for `/land` or committer when the user asks.
- `make check && make test && make lint` must stay green after each task.

## File map

| File | Responsibility |
| --- | --- |
| `src/utils/process.rs` | Process-group configure; cancel-aware `stream_and_wait`; TERM/KILL helpers |
| `src/engine/commands/run.rs` | `configure_command` before spawn; map cancel → `Error::Aborted` |
| `src/engine/commands/clone.rs` | Same as run for git |
| `src/engine/event.rs` | `RunCancelling`, `TaskCancelled`, `AllDone.cancelled` |
| `src/engine/runner.rs` | Emit cancel events; tally cancelled; avoid abort-before-kill |
| `src/engine/commands/tree.rs` | Optional cancel polls between files/chunks |
| `src/engine/commands/fs_ops.rs` | Thread cancel into `apply_tree_*` |
| `src/engine/commands/tree_op.rs` | Pass `&ctx.cancel` into apply_tree |
| `src/tui/state.rs` | `TaskStatus::Cancelled`; `cancelled` counter; `completed_tasks` |
| `src/tui/reduce.rs` | Handle new events |
| `src/tui/widgets/{header,task_list}.rs` | Cancelling strip; cancelled badge |
| `src/tui/details/render.rs` | Cancelled status spans |
| `src/tui/plain.rs` | Plain cancel lines |
| `src/tui/mod.rs` | Summary + exit on cancelled |
| `src/engine/sink.rs` | Fix `AllDone` construction in tests |
| `CONTEXT.md` / `README.md` / `CHANGELOG.md` | Docs |

---

### Task 1: Capture Criterion baseline (before code)

**Files:**
- Create (local only, do not commit): `/tmp/machine_setup_bench/cancel-baseline.txt`

**Interfaces:**
- Produces: baseline Criterion text for later Task 7 comparison
- Consumes: none

- [ ] **Step 1: Run default 1k Command bench on clean tree**

```bash
mkdir -p /tmp/machine_setup_bench
cargo bench --bench command_bench 2>&1 | tee /tmp/machine_setup_bench/cancel-baseline.txt
```

Expected: Criterion finishes; file contains `tree_install_direct/1k_files`, `mtime_skip/1k`, `runner_smoke/…`.

- [ ] **Step 2: Record key means** (paste into the work log / PR notes)

From the tee output, note mean times for at least:
- `tree_install_direct/1k_files`
- `tree_uninstall_direct/1k_files`
- `mtime_skip/1k`
- `runner_smoke/single_copy_1k_null_sink`
- `runner_smoke/parallel_two_copy_tasks_1k`

No code changes in this task.

---

### Task 2: Process-group spawn + cancel-aware `stream_and_wait`

**Files:**
- Modify: `src/utils/process.rs`
- Modify: `src/engine/commands/run.rs` (only if needed to compile shared helpers — prefer keep spawn config in process.rs)
- Modify: `src/engine/commands/clone.rs` (same)

**Interfaces:**
- Produces:
  - `pub const CANCEL_GRACE: Duration = Duration::from_secs(2);`
  - `pub fn configure_command(cmd: &mut tokio::process::Command);` — Unix: `pre_exec` → `setsid()` (ignore EPERM if already session leader); Windows: best-effort creation flags / documented tree-kill prep
  - `pub async fn stream_and_wait(child, ctx, options) -> crate::error::Result<std::process::ExitStatus>;` — on cancel: tear down process group, return `Err(Error::Aborted)`; on wait IO error: `Err(Error::Io(…))` or existing mapping; success: `Ok(status)`
  - Internal: `async fn teardown_child(child: &mut Child) -> …` — TERM group → wait up to `CANCEL_GRACE` → KILL
- Consumes: `ctx.cancel: CancellationToken`; existing `CommandContext` logging

- [ ] **Step 1: Write failing tests** in `src/utils/process.rs` `#[cfg(test)]` (Unix-focused; `#[cfg(unix)]`):

```rust
#[cfg(unix)]
#[tokio::test]
async fn cancel_kills_shell_and_grandchild_sleep() {
    use std::time::Duration;
    use tokio::process::Command;
    use tokio_util::sync::CancellationToken;

    // Build a minimal CommandContext with NullSink + cancel token
    // (mirror other engine tests — see runner/context test helpers).
    let cancel = CancellationToken::new();
    let ctx = /* test CommandContext with cancel.clone() */;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("sleep 300 & wait");
    configure_command(&mut cmd);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = cmd.spawn().expect("spawn");

    let wait = tokio::spawn({
        let ctx = ctx.clone();
        async move { stream_and_wait(child, &ctx, StreamOptions::interactive()).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.cancel();
    let err = wait.await.expect("join").expect_err("must abort");
    assert!(matches!(err, crate::error::Error::Aborted));
    // Optional: assert no leftover `sleep 300` from this test via `pgrep` scoped by pgid if stable.
}
```

Also add a unit test that `configure_command` is callable on a `Command` without panicking.

- [ ] **Step 2: Run tests — expect fail**

```bash
cargo test -q --lib cancel_kills_shell_and_grandchild_sleep
```

Expected: FAIL (compile error or old `stream_and_wait` never returns `Aborted`).

- [ ] **Step 3: Implement**

1. Change `stream_and_wait` signature to `crate::error::Result<ExitStatus>`.
2. After taking stdout/stderr handles, `tokio::select!` between `child.wait()` and `ctx.cancel.cancelled()`.
3. On cancel branch: call teardown (Unix `libc::kill(-pgid, SIGTERM)`, then `tokio::time::timeout(CANCEL_GRACE, child.wait())`, then `SIGKILL` if needed; use `child.id()` as pgid after `setsid`). Then return `Err(Error::Aborted)`.
4. Implement `configure_command`:

```rust
pub fn configure_command(cmd: &mut tokio::process::Command) {
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // New session ⇒ new process group with pgid == pid.
            if libc::setsid() == -1 {
                let err = std::io::Error::last_os_error();
                // EPERM: already a session leader — acceptable.
                if err.raw_os_error() != Some(libc::EPERM) {
                    return Err(err);
                }
            }
            Ok(())
        });
    }
    #[cfg(windows)]
    {
        // Best-effort: CREATE_NEW_PROCESS_GROUP if available via windows-sys,
        // or document + terminate with taskkill /T /PID on cancel.
        let _ = cmd; // replace with real flags / kill helper
    }
}
```

5. Update `run.rs` / `clone.rs` call sites: `configure_command(&mut cmd)` before spawn; map `stream_and_wait` errors — `Error::Aborted` passthrough; other errors keep `ShellFailed` / `GitFailed` wrappers **only for non-Aborted**.

Example mapping in `run.rs`:

```rust
let status = match process::stream_and_wait(child, ctx, options).await {
    Ok(s) => s,
    Err(Error::Aborted) => return Err(Error::Aborted),
    Err(e) => return Err(Error::ShellFailed(format!("Failed to wait for shell: {e}"))),
};
```

- [ ] **Step 4: Run tests**

```bash
cargo test -q --lib cancel_kills_shell -- --nocapture
cargo test -q --lib output_line_buffer
make check
```

Expected: PASS.

- [ ] **Step 5: Do not commit** (leave uncommitted for later `/land`).

---

### Task 3: Events + Runner cancel tally

**Files:**
- Modify: `src/engine/event.rs`
- Modify: `src/engine/runner.rs`
- Modify: `src/engine/sink.rs` (test `AllDone` literals)
- Modify: any exhaustiveness breakages (`plain.rs`, `reduce.rs` — stub minimally so it compiles; full UX in Task 5)

**Interfaces:**
- Produces:
  - `TaskEvent::RunCancelling`
  - `TaskEvent::TaskCancelled { task_name: Arc<str> }`
  - `AllDone { succeeded, failed, skipped, cancelled: usize }`
  - `Tally { …, cancelled: usize }`
- Consumes: `Error::Aborted` from tasks; `self.cancel`

- [ ] **Step 1: Write failing Runner/event unit tests**

Add tests (in `runner.rs` or a focused test module) that when cancel fires mid-run:
- exactly one `RunCancelling` is observed (or at least one before `AllDone`)
- running task yields `TaskCancelled` not `TaskFailed`
- `AllDone.cancelled >= 1`
- `run_tasks` returns `Err(Error::Aborted)`

Use a tiny config with a long `run: sleep 30` Task, NullSink channel, cancel after `TaskStarted`.

- [ ] **Step 2: Run — expect fail** (variants missing).

- [ ] **Step 3: Implement**

1. Extend `TaskEvent` + `AllDone`.
2. In `run_tasks`: when `cancel` first observed as cancelled (start of loop or layer), `self.send(TaskEvent::RunCancelling)` once (use a `bool run_cancelling_emitted` local).
3. In `run_layer` join handling:
   - `Err(Error::Aborted)` → emit `TaskCancelled`, `tally.cancelled += 1` (not failed).
   - Remove the JoinSet arm that returns `Err(Aborted)` **without** letting `stream_and_wait` teardown: prefer waiting on the task future so cooperative cancel wins. If keep `select!`, ensure the future is polled until teardown completes — e.g. drop the race-abort arm and rely on inner cancel, **or** on cancel wait for `run_task_with_retry` to finish after token fire (with timeout).
4. Pass `cancelled` into `AllDone`.
5. Fix compile errors in TUI/plain with temporary `_` arms that ignore or treat like failed — Task 5 replaces them.

Recommended JoinSet shape:

```rust
join_set.spawn(async move {
    let result = run_task_with_retry(&task_config, &ctx, executors).await;
    (name_arc, result)
});
// Do NOT select! abort the future before run_task returns.
// run_task / stream_and_wait observe ctx.cancel.
```

- [ ] **Step 4: Run**

```bash
cargo test -q --lib
make lint
```

Expected: PASS (TUI may temporarily show weak cancel UX).

- [ ] **Step 5: Do not commit.**

---

### Task 4: Tree / File ops cancel polls

**Files:**
- Modify: `src/engine/commands/tree.rs`
- Modify: `src/engine/commands/fs_ops.rs`
- Modify: `src/engine/commands/tree_op.rs`
- Update call sites that break: benches / tests passing `None` for cancel

**Interfaces:**
- Produces:
  - `install_tree_with_pool(..., cancel: Option<&CancellationToken>)`
  - `uninstall_tree_with_pool(..., cancel: Option<&CancellationToken>)`
  - Same for `*_gated` and `apply_tree_install` / `apply_tree_uninstall`
  - On cancel: `Err(Error::Aborted)` before applying the next file / before next chunk flush
- Consumes: `ctx.cancel` from `tree_op::run_sync`

- [ ] **Step 1: Write failing tree test**

```rust
#[test]
fn install_tree_stops_between_files_when_cancelled() {
    let cancel = CancellationToken::new();
    // Build a multi-file fixture (e.g. 20 files).
    // on_file: after first successful apply, cancel.cancel(); count applies.
    // install_tree_with_pool(..., Some(&cancel))
    // assert Err(Aborted) and applied_count < total_files
}
```

- [ ] **Step 2: Run — expect fail.**

- [ ] **Step 3: Implement**

In walk/chunk loops and sequential `apply_files` / `apply_dests`:

```rust
if cancel.is_some_and(|c| c.is_cancelled()) {
    return Err(Error::Aborted);
}
```

Check:
- before each `on_file` in sequential apply
- before each chunk `apply_files` flush
- at start of each `walk_relative` file entry (stream apply path)
- parallel `par_iter` path: check cancel in `try_for_each` closure (best-effort; some in-flight Rayon tasks may finish)

Thread `Some(&ctx.cancel)` from `tree_op` → `fs_ops::apply_tree_*` → `tree::*`.
Benches and unit tests that call tree helpers: pass `None`.

- [ ] **Step 4: Run**

```bash
cargo test -q --lib install_tree_stops_between_files
cargo test -q --lib tree::
make check
```

Expected: PASS.

- [ ] **Step 5: Do not commit.**

---

### Task 5: TUI + plain cancel UX

**Files:**
- Modify: `src/tui/state.rs`
- Modify: `src/tui/reduce.rs`
- Modify: `src/tui/widgets/header.rs`
- Modify: `src/tui/widgets/task_list.rs`
- Modify: `src/tui/details/render.rs`
- Modify: `src/tui/plain.rs`
- Modify: `src/tui/mod.rs` (`print_summary`, exit)
- Modify: `src/tui/parallel_burst.rs` if status matching needs Cancelled

**Interfaces:**
- Produces:
  - `TaskStatus::Cancelled`
  - `UiState.cancelled: usize`
  - `UiState.cancelling: bool` (set on `RunCancelling`)
  - Header: while `cancelling && !done` → ` Cancelling… · {elapsed} ` (keep counts if useful)
  - Done strip includes `K cancelled` when `cancelled > 0`
  - List glyph for Cancelled: warning color (e.g. `⊘` or `×`), not error red
  - `print_summary` lists cancelled names; exit if `cancelled > 0 || failed > 0`
  - Plain: `!! Cancelling…`, `xx Cancelled: {task}`, AllDone includes cancelled
- Consumes: new `TaskEvent` variants from Task 3

- [ ] **Step 1: Write failing reduce/header tests**

```rust
#[test]
fn task_cancelled_sets_status_and_count() { /* … */ }

#[test]
fn run_cancelling_sets_flag() { /* … */ }

#[test]
fn completion_strip_includes_cancelled() {
    // unit-test pure format helper if extracted, or header status string builder
}
```

- [ ] **Step 2: Run — expect fail.**

- [ ] **Step 3: Implement** state/reduce/widgets/plain/mod per Interfaces. Update `is_done()`:

```rust
matches!(
    self,
    TaskStatus::Completed
        | TaskStatus::Failed(_)
        | TaskStatus::Skipped(_)
        | TaskStatus::Cancelled
)
```

`completed_tasks()` must count Cancelled as completed for the gauge.

- [ ] **Step 4: Run**

```bash
cargo test -q --lib tui::
make test
make lint
```

Expected: PASS.

- [ ] **Step 5: Do not commit.**

---

### Task 6: Docs + after-bench comparison

**Files:**
- Modify: `CONTEXT.md` — Runner cancel tears down process groups; File ops poll; remove “OS subprocess teardown is a follow-up”
- Modify: `README.md` — brief `q` / Ctrl+C cancel note
- Modify: `CHANGELOG.md` — `[Unreleased]` Added/Changed + Performance note
- Local: `/tmp/machine_setup_bench/cancel-after.txt`

**Interfaces:**
- Consumes: Task 1 baseline file
- Produces: documented behavior + bench delta note

- [ ] **Step 1: Run after bench**

```bash
cargo bench --bench command_bench 2>&1 | tee /tmp/machine_setup_bench/cancel-after.txt
```

- [ ] **Step 2: Compare** baseline vs after for the five means from Task 1. Soft gate: if `tree_install_direct/1k` or `runner_smoke/*` regresses **>~5%**, re-run isolated (clean worktree vs dirty) before claiming a regression; only then investigate code.

- [ ] **Step 3: Update docs**

CONTEXT.md Runner bullet — replace follow-up sentence with process-group + File ops poll language (use CONTEXT vocabulary).

CHANGELOG example:

```markdown
### Added
- Cancel tears down OS process groups for `run` / `clone` (TERM → grace → KILL) and polls Tree apply between files/chunks
- Distinct cancel UX: Cancelling / Cancelled in TUI and plain mode (`AllDone.cancelled`)

### Performance
- Command bench (1k) vs pre-change baseline: <one line summary>
```

- [ ] **Step 4: Final verify**

```bash
make check && make test && make lint
```

Expected: green.

- [ ] **Step 5: Do not commit** — hand off to `/land` or user-requested committer with bench summary in the message body.

---

## Spec coverage (self-review)

| Spec requirement | Task |
| --- | --- |
| Process group / tree kill + TERM grace KILL | 2 |
| Cooperative wait owns kill | 2, 3 |
| Tree polls between files/chunks | 4 |
| `RunCancelling` / `TaskCancelled` / `AllDone.cancelled` | 3 |
| TUI badges + summary + plain | 5 |
| Non-zero exit on cancel | 5 |
| History unchanged for non-completed | 3 (no change to update_history on Aborted) |
| Before/after Criterion soft gate | 1, 6 |
| CONTEXT / README / CHANGELOG | 6 |
| Unix grandchild test | 2 |
| Windows best-effort | 2 |

## Placeholder scan

None intentional. Windows branch must still implement a real best-effort kill (not a permanent `let _ = cmd`).

## Type consistency

- `stream_and_wait` → `crate::error::Result<ExitStatus>` with `Error::Aborted`
- `AllDone.cancelled: usize` everywhere (runner, sink tests, reduce, plain)
- Tree APIs take `Option<&CancellationToken>` consistently through fs_ops → tree_op
