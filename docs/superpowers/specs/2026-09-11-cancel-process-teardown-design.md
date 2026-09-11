# Cancel: process teardown + File ops polls + cancel UX

Date: 2026-09-11  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **1** — cancel-aware `stream_and_wait` + tree cancel polls  
Related: `CONTEXT.md` (Runner cancel follow-up), ADR-0005 (Task event sink),
ADR-0009 (Runner grid), Command bench (ADR-0001 report-only)

## Context

The Runner shares a `CancellationToken` with nested Sub-config Runners. Cancel
(`q` / Ctrl+C) aborts in-flight Tokio tasks via `JoinSet`, but OS children from
`run` and `clone` are not torn down — shell grandchildren (`brew`, `apt`, …)
can linger. File ops / Tree materialization also keep applying until the
current chunk finishes with no cancel poll. Cancelled work is surfaced as
`TaskFailed` / `Error::Aborted`, which looks like a normal failure.

`CONTEXT.md` already names OS subprocess teardown as a follow-up to the
async deepenings.

## Goals

- On cancel, nothing we started lingers: process-group (Unix) / process-tree
  best-effort (Windows) teardown for spawned children, with **SIGTERM → ~2s
  grace → SIGKILL** (or platform equivalents).
- Cooperative cancel inside `stream_and_wait` owns kill; JoinSet abort must
  not race ahead of teardown.
- Tree / File ops poll cancel **between files and between PathBuf chunks**;
  stop new apply admission; the current single-file op may finish.
- Distinct cancel UX: `TaskCancelled`, list + Runner grid badges, header
  Cancelling → Cancelled, end summary with cancelled count, plain-mode lines;
  non-zero exit; History unchanged for non-completed Tasks.
- Local Criterion **before/after** on default 1k Command-bench + runner smoke
  (report-only); investigate if install/runner smoke regresses **>~5%**.

## Non-goals

- Hard-stop mid-`std::fs` single-file copy or mid-Rayon per-file apply.
- Soft Criterion baselines in CI (still ADR-0001).
- Full ratatui snapshot suite (still ADR-0009).
- Comment-preserving YAML, plugins, Exclusive-lane / sudo batching redesign.
- Criterion microbench of the cancel path itself.

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Correctness | Kill everything we spawn (process group / tree), not direct child only |
| Signal | TERM → ~2s grace → KILL |
| File ops | Poll between files/chunks (Approach B); current file may finish |
| Architecture | Approach 1: central cancel-aware `stream_and_wait` + tree polls |
| JoinSet | Prefer cooperative cancel in wait path; avoid abort-before-kill |
| Events | `RunCancelling`, `TaskCancelled`; `AllDone.cancelled` |
| TUI status | `TaskStatus::Cancelled` — done, not failed |
| History | Only `TaskCompleted` updates install record (unchanged) |
| Exit | Non-zero if cancelled or failed |
| Bench | Before/after 1k Criterion + runner smoke; soft ~5% human gate |

## Architecture

```text
CancelToken.cancel()
        │
        ├─ emit RunCancelling (once)
        │
        ├─ stream_and_wait (run / clone)
        │     select! wait vs cancelled
        │     → TERM process group → grace → KILL
        │     → Aborted / cancel result (not ShellFailed/GitFailed)
        │
        ├─ tree apply / File ops
        │     check cancel between files + between chunks
        │     → Error::Aborted
        │
        └─ Runner tallies
              Running → TaskCancelled
              AllDone { succeeded, failed, skipped, cancelled }
```

`utils/process.rs` owns spawn group setup + teardown. `run` and `clone`
keep calling `stream_and_wait`. Tree materialization gains an optional cancel
hook (or `&CancellationToken`) on gated install/uninstall paths used by
production executors; benches may pass a never-cancelled token / `None`.

## Process teardown

**Unix**

- After spawn (or via `pre_exec`): child is leader of a new process group.
- On cancel: `kill(-pgid, SIGTERM)`, wait up to **2 seconds** for exit, then
  `kill(-pgid, SIGKILL)` if still alive.
- Reap the `Child` and join stdout/stderr stream tasks.

**Windows**

- Best-effort process-tree terminate for the spawned process (document limits:
  may not cover every job-object edge case).
- Same grace window conceptually (terminate → short wait → force if needed).

**Uncontended path**

- Extra cost is process-group setup at spawn only; no polling on the happy
  wait path beyond the existing `select!` with cancel (cheap).

## Events & Runner

New / changed Task events:

- `RunCancelling` — once when the run token fires (header / plain cue).
- `TaskCancelled { task_name: Arc<str> }` — Task was Running and stopped by
  cancel (not `TaskFailed`).
- `AllDone { succeeded, failed, skipped, cancelled }` — add `cancelled`.

Runner behavior:

- Prefer executor-local cancel (wait path / tree polls) so kill runs before
  the task future is dropped.
- Tasks that never left Pending are not counted as cancelled.
- Map cancel results to `TaskCancelled`; do not emit `TaskFailed` with
  `"Aborted"` for user cancel.
- Nested Sub-config shares the parent token; teardown applies at each spawn.

## TUI / plain

**Interactive**

- `RunCancelling` → header **Cancelling…** (keep progress counts).
- `TaskCancelled` → list + Runner grid badge (warning color, not error red);
  log line `Cancelled.`; `TaskStatus::Cancelled` is `is_done()`, not
  `is_failed()`.
- Completion strip / `print_summary`: include cancelled count and list
  cancelled task names; omit zero segments like today.
- Help bar: unchanged `q` cancel while running; Esc quit when done.
- Exit non-zero if `cancelled > 0` or `failed > 0`.

**Plain / `--no-tui`**

- `!! Cancelling…` on `RunCancelling`.
- `xx Cancelled: {task}` on `TaskCancelled`.
- `AllDone` line includes cancelled count.

## Testing

- Unit: cancel helpers with a long-running child (and grandchild under a
  shell when feasible) — no linger after cancel; grace then hard kill.
- Tree: cancel between files stops further applies.
- Reduce / header / summary / plain for Cancelled + `AllDone.cancelled`.
- Existing cancel / abort tests updated to the new event shape.
- No full ratatui snapshot suite.

## Performance / bench

1. On clean HEAD before implementation: `cargo bench --bench command_bench`
   (default 1k) — capture `tree_*`, `mtime_skip`, `runner_smoke`, startup /
   registry as available.
2. After implementation: same command; compare deltas.
3. Soft gate: if `tree_install_direct/1k` or `runner_smoke/*` regresses
   **>~5%**, investigate (isolated worktree / thermal noise) before landing.
4. Report deltas in CHANGELOG Performance subsection if notable; otherwise a
   short “no meaningful regression” note in the PR / commit body is enough.
5. Cancel path itself is not a Criterion case (ADR-0001 report-only).

## Docs

- Spec: this file.
- `CONTEXT.md`: Runner cancel tears down OS process groups; File ops poll
  between files/chunks; remove “OS subprocess teardown is a follow-up.”
- README: brief note on `q` / Ctrl+C cancel behavior.
- CHANGELOG: Added/Changed + optional Performance note.

## Error / edge cases

- Cancel before any Task starts: `RunCancelling` + empty/`0` cancelled; exit
  non-zero.
- Cancel during Exclusive-lane wait: Task becomes Cancelled; lane released.
- SudoFs script batch: cancel between batches / files; do not leave an
  orphaned sudo helper if we spawned one (same process-group rules if it is
  our child).
- Double Ctrl+C: token already cancelled; teardown idempotent.
- Windows tree kill best-effort: document; prefer no false “clean” claim if
  terminate fails — still return cancel and surface errors in logs if kill
  fails.

## Success criteria

- Cancel leaves no lingering OS children from `run` / `clone` in the normal
  Unix case (verified by test with sleep/grandchild).
- Tree apply stops scheduling further files after cancel.
- TUI/plain show Cancelling / Cancelled distinctly from failure.
- `make check && make test && make lint` green.
- Before/after Criterion captured; no unexplained >~5% hit on install /
  runner smoke.
