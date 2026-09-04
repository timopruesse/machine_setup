# Tree materialization memory baseline (measure before chunking)

Date: 2026-09-04  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **1** — Criterion fixture ladder + separate opt-in peak-RSS memory harness; chunked apply stays deferred until the gate fires

## Context

ADR-0004 keeps install/uninstall as collect-full-list then apply (mkdir parents
during WalkDir, then `apply_files` / `apply_dests`). Chunked or streaming apply
is deferred until a large Command-bench fixture (or real Config) shows peak
`PathBuf` / allocator pain worth complicating that invariant.

Today `benches/command_bench.rs` is wall-clock only on a **1k**-file fixture
(ADR-0001 report-only). There is no peak-RSS or PathBuf-list estimate, so the
ADR-0004 gate cannot be evaluated.

## Goals

- Add a **fixture ladder**: 1k (default Criterion), 10k (opt-in wall-clock),
  100k (memory harness only).
- Keep Criterion **report-only** (no CI fail on ms budgets).
- Add a **separate** memory harness that prints peak RSS, a PathBuf list
  estimate, and `PASS` / `RECOMMEND_CHUNK` against documented thresholds.
- Update ADR-0004 with how to run the ladder and when to reopen chunking.
- Unit-test pure helpers (size parse, estimate math, verdict).

## Non-goals

- Implementing chunked / streaming apply.
- Soft Criterion baseline comparison in CI (ADR-0001 follow-up).
- SudoFs in the memory harness (DirectFs only; sudo stays behind existing
  `MACHINE_SETUP_BENCH_SUDO` for Criterion).
- Hard CI failure on RSS or gate verdict.
- `dhat` / custom `GlobalAlloc` instrumentation.
- Changing production `tree.rs` collect-then-apply behavior.

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Strategy | Measure first; design chunking only if harness says `RECOMMEND_CHUNK` |
| Wall-clock sizes | Criterion: `1000` default; `10000` via env |
| Memory size | Harness only: `100_000` files |
| Runner smoke | Always **1k** (10k opt-in does not enlarge runner_smoke) |
| Tree_* groups under 10k | `tree_install_*`, `mtime_skip`, `tree_symlink_*`, `tree_uninstall_*` use selected size |
| Memory tool | Separate example/bin, not a Criterion group |
| RSS | `getrusage(RUSAGE_SELF).ru_maxrss` on macOS/Linux; normalize units |
| Windows RSS | Unsupported → print `UNSUPPORTED`; PathBuf estimate still runs; gate is PathBuf-only |
| PathBuf estimate | `n_files * 2 * avg_path_bytes` from fixture src+dest path lengths |
| Gate (100k DirectFs install) | Peak RSS ≥ **256 MiB** **or** PathBuf estimate ≥ **64 MiB** → `RECOMMEND_CHUNK` |
| Exit code | Always 0 for PASS/RECOMMEND_CHUNK (report-only); non-zero only on harness failure |
| Threshold source of truth | Constants in the harness; ADR-0004 cites the same numbers |
| Sudo in harness | Out of scope |
| Optional uninstall in harness | `--uninstall` flag; default is install-only |

## Behavior

### Criterion (`benches/command_bench.rs`)

1. Read `MACHINE_SETUP_BENCH_TREE_SIZE`:
   - unset / empty → `1000`
   - `1000` or `10000` → that size
   - anything else → panic with allowed values
2. Generate-once fixture at the selected size (reuse existing
   `TreeFixture::generate` layout: `src/`, `src/a/`, `src/a/b/`, `f{i}.txt`).
3. Name bench functions by size (`1k_files` / `10k_files`).
4. `runner_smoke` and startup/registry groups stay unchanged (1k / no tree).

### Memory harness (`examples/tree_memory_harness.rs`)

Ship as a Cargo example (add `required-features` / path deps only if the
workspace already does that for examples; otherwise plain `[[example]]`).
Run with release for meaningful RSS:

```bash
cargo run --example tree_memory_harness --release
cargo run --example tree_memory_harness --release -- --uninstall
```

Flow:

1. Create temp roots; generate **100k** file tree (same layout as Criterion).
2. Record peak RSS baseline (optional) and run one DirectFs
   `install_tree_with_pool` with the shared-style Rayon pool (same threshold
   rules as production).
3. Read peak RSS after install; compute PathBuf estimate from measured average
   src/dest path byte lengths × `n_files * 2`.
4. Print plain lines (stable keys for grepping), e.g.:

   ```
   n_files=100000
   peak_rss_mib=...
   pathbuf_estimate_mib=...
   verdict=PASS|RECOMMEND_CHUNK
   ```

5. If `--uninstall`: after install, uninstall with pool and print a second
   RSS/estimate block labeled `phase=uninstall` (install gate remains the
   ADR decision signal).
6. Exit 0 when the run completed; exit non-zero on fixture/install errors.

### Peak RSS units

- Linux: `ru_maxrss` is KiB → MiB = value / 1024.
- macOS: `ru_maxrss` is bytes → MiB = value / (1024 * 1024).
- Document the normalization next to the constants.

### ADR-0004 update

Replace the short deferred paragraph with:

- Why collect-then-apply remains.
- Ladder + env + harness command.
- Threshold table and `PASS` / `RECOMMEND_CHUNK`.
- Explicit: chunked apply stays deferred until a local harness run recommends it
  (or a real Config document shows pain).

### CONTEXT.md

One short vocabulary note under Command bench: wall-clock Criterion vs opt-in
memory harness; both report-only.

## Testing

| Kind | What |
| --- | --- |
| Unit | Env size parse; PathBuf estimate; verdict helper |
| Manual | `cargo bench` (1k); `MACHINE_SETUP_BENCH_TREE_SIZE=10000 cargo bench --bench command_bench` (local); release harness on macOS/Linux |
| CI | No 10k Criterion; no 100k harness; no gate assert |

## Error handling

- Invalid Criterion tree size → panic with allowed values (bench process).
- Harness I/O or install failure → stderr + non-zero exit.
- Windows without RSS → `peak_rss_mib=UNSUPPORTED`; verdict from PathBuf estimate only.

## Follow-up (out of this change)

- If harness prints `RECOMMEND_CHUNK`, open a new design for chunked/streaming
  apply that preserves mkdir-then-apply.
- Soft Criterion baseline comparison in CI (ADR-0001).
- Optional `--strict` exit code on `RECOMMEND_CHUNK` (not required now).

## Self-review checklist

- No TBD placeholders; thresholds and env names are concrete.
- Scope is measure-only; production tree apply unchanged.
- Gate numbers live in harness constants and are mirrored in ADR-0004.
