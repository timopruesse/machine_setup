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

## Deferred: chunked / streaming file lists

Install/uninstall still collect the full file (or dest) list, then apply.
Chunked or streaming apply (to cut peak `PathBuf` memory on huge trees) stays
**deferred** until the memory harness recommends it (or a real Config document
shows pain).

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
Reopen chunked apply only after a local harness run recommends it. Report-only;
not enforced in CI. SudoFs is out of scope for the harness (`MACHINE_SETUP_BENCH_SUDO`
remains Criterion-only).
