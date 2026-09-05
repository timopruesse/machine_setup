# Parallel file apply inside Tree materialization

Directory installs create parents sequentially (WalkDir order), then apply
collected files on the **shared** Rayon pool owned by the Concurrency gate
(sized by `num_threads`, default: physical CPUs − 1; created on first pool
use). Sibling Command entries share that pool so `parallel: true` tasks do
not oversubscribe. Threshold:
fewer than 32 files stay sequential. SudoFs walks stay single-threaded (script
batch on flush). Symlink stays sequential (metadata-cheap) — remeasured 2026-08
(DirectFs 1k-file install: parallel ~2.5× slower than sequential). No second
concurrency knob.

## Chunked file lists (accepted 2026-09-05 — not yet implemented)

**Decision:** land **hybrid single-pass capped collect** inside Tree
materialization for **all** list-collecting paths (DirectFs/SudoFs copy and
symlink install/uninstall).

- Grow the in-memory path list until the next entry would push the PathBuf
  list estimate over **`PATHBUF_ESTIMATE_GATE_MIB` (64 MiB)** (constant in
  `tree_measure.rs`); flush/apply that chunk; repeat.
- Trees whose whole list stays under the gate remain a **single chunk** —
  same observable behavior as today’s collect-then-apply for normal configs
  and the 1k/10k Command bench ladder.
- Full streaming (never buffering a chunk) and a separate raw file-count
  threshold were rejected.
- Memory harness stays the regression check (RSS + estimate + verdict); it is
  no longer a gate that must fire before implementation.

### How to measure

| Ladder | How |
| --- | --- |
| 1k wall-clock | `cargo bench --bench command_bench` (default) |
| 10k wall-clock | `MACHINE_SETUP_BENCH_TREE_SIZE=10000 cargo bench --bench command_bench` |
| 100k memory | `cargo run --example tree_memory_harness --release` |

Gate constants (source of truth: `src/engine/commands/tree_measure.rs`):

- Peak RSS ≥ **256 MiB** on the 100k DirectFs install, **or**
- PathBuf list estimate ≥ **64 MiB**

→ harness prints `verdict=RECOMMEND_CHUNK` (exit 0). Otherwise `verdict=PASS`.
Report-only; not enforced in CI. SudoFs remains out of scope for the harness
(`MACHINE_SETUP_BENCH_SUDO` is Criterion-only).
