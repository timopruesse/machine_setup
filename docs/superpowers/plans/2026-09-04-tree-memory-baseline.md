# Tree memory baseline (measure before chunking) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Criterion fixture ladder (1k default / 10k opt-in) and a separate 100k peak-RSS memory harness with an explicit `PASS` / `RECOMMEND_CHUNK` gate so ADR-0004 can decide whether chunked apply is worth it — without implementing chunking yet.

**Architecture:** Pure measurement helpers live in a small library module (`tree_measure`) shared by Criterion and an example harness. Criterion keeps report-only wall-clock benches; the example prints peak RSS + PathBuf list estimate and a documented verdict. Production `tree.rs` collect-then-apply is unchanged.

**Tech Stack:** Rust, Criterion, Rayon (existing), `libc` for `getrusage`, tempfile, Cargo example.

**Spec:** `docs/superpowers/specs/2026-09-04-tree-memory-baseline-design.md`

## Global Constraints

- Measure-only: no chunked/streaming apply in `tree.rs`.
- Criterion remains report-only (ADR-0001): no CI fail on ms or RSS.
- Criterion tree sizes: only `1000` (default) and `10000` via `MACHINE_SETUP_BENCH_TREE_SIZE`.
- Memory harness: `100_000` files, DirectFs only, exit 0 on `PASS`/`RECOMMEND_CHUNK`.
- Gate (100k install): peak RSS ≥ **256 MiB** **or** PathBuf estimate ≥ **64 MiB** → `RECOMMEND_CHUNK`.
- `runner_smoke` stays on 1k even when Criterion tree size is 10k.
- Threshold constants live in `tree_measure`; ADR-0004 cites the same numbers.
- Do not commit unless the user explicitly asks.

## File map

| File | Responsibility |
| --- | --- |
| `src/engine/commands/tree_measure.rs` | Size parse, PathBuf estimate, verdict, peak RSS (macOS/Linux), gate constants |
| `src/engine/commands/mod.rs` | `pub mod tree_measure` |
| `Cargo.toml` | Direct `libc` dep; optional `[[example]]` if needed |
| `benches/command_bench.rs` | Fixture ladder via env; size-named tree benches; runner_smoke fixed at 1k |
| `examples/tree_memory_harness.rs` | 100k DirectFs install (+ optional `--uninstall`); print metrics + verdict |
| `docs/adr/0004-defer-parallel-tree-materialization.md` | Ladder, harness command, thresholds |
| `CONTEXT.md` | Command bench vocabulary: wall-clock vs memory harness |
| `CHANGELOG.md` | `[Unreleased]` Added note for ladder + harness |

---

### Task 1: `tree_measure` helpers (TDD)

**Files:**
- Create: `src/engine/commands/tree_measure.rs`
- Modify: `src/engine/commands/mod.rs`
- Modify: `Cargo.toml` (add `libc = "0.2"` under `[dependencies]`)

**Interfaces:**
- Produces:
  - `pub const CRITERION_DEFAULT_FILES: usize = 1_000;`
  - `pub const CRITERION_LARGE_FILES: usize = 10_000;`
  - `pub const MEMORY_HARNESS_FILES: usize = 100_000;`
  - `pub const PEAK_RSS_GATE_MIB: f64 = 256.0;`
  - `pub const PATHBUF_ESTIMATE_GATE_MIB: f64 = 64.0;`
  - `pub fn parse_bench_tree_size(raw: Option<&str>) -> Result<usize, String>`
  - `pub fn pathbuf_list_estimate_mib(n_files: usize, avg_path_bytes: f64) -> f64`
  - `pub enum GateVerdict { Pass, RecommendChunk }` (+ `Display` as `PASS` / `RECOMMEND_CHUNK`)
  - `pub fn gate_verdict(peak_rss_mib: Option<f64>, pathbuf_estimate_mib: f64) -> GateVerdict`
  - `pub fn peak_rss_mib() -> Option<f64>` — `None` on unsupported platforms

- [ ] **Step 1: Write failing unit tests** in `tree_measure.rs` under `#[cfg(test)] mod tests`:

```rust
#[test]
fn parse_default_and_allowed() {
    assert_eq!(parse_bench_tree_size(None).unwrap(), 1_000);
    assert_eq!(parse_bench_tree_size(Some("")).unwrap(), 1_000);
    assert_eq!(parse_bench_tree_size(Some("1000")).unwrap(), 1_000);
    assert_eq!(parse_bench_tree_size(Some("10000")).unwrap(), 10_000);
}

#[test]
fn parse_rejects_unknown() {
    assert!(parse_bench_tree_size(Some("100000")).is_err());
    assert!(parse_bench_tree_size(Some("5000")).is_err());
}

#[test]
fn pathbuf_estimate_scales() {
    // 100_000 files * 2 paths * 100 bytes = 20_000_000 bytes ≈ 19.07 MiB
    let mib = pathbuf_list_estimate_mib(100_000, 100.0);
    assert!((mib - 20_000_000.0 / (1024.0 * 1024.0)).abs() < 0.01);
}

#[test]
fn verdict_rss_or_pathbuf() {
    assert!(matches!(
        gate_verdict(Some(255.0), 63.0),
        GateVerdict::Pass
    ));
    assert!(matches!(
        gate_verdict(Some(256.0), 0.0),
        GateVerdict::RecommendChunk
    ));
    assert!(matches!(
        gate_verdict(None, 64.0),
        GateVerdict::RecommendChunk
    ));
    assert!(matches!(
        gate_verdict(None, 63.9),
        GateVerdict::Pass
    ));
}
```

- [ ] **Step 2: Run tests — expect fail** (module missing)

```bash
cargo test -p machine_setup tree_measure -- --nocapture
```

Expected: compile error / no such module.

- [ ] **Step 3: Implement module**

```rust
//! Measurement helpers for Command-bench tree ladder + ADR-0004 memory gate.

use std::fmt;

pub const CRITERION_DEFAULT_FILES: usize = 1_000;
pub const CRITERION_LARGE_FILES: usize = 10_000;
pub const MEMORY_HARNESS_FILES: usize = 100_000;
pub const PEAK_RSS_GATE_MIB: f64 = 256.0;
pub const PATHBUF_ESTIMATE_GATE_MIB: f64 = 64.0;

pub fn parse_bench_tree_size(raw: Option<&str>) -> Result<usize, String> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(CRITERION_DEFAULT_FILES),
        Some("1000") => Ok(CRITERION_DEFAULT_FILES),
        Some("10000") => Ok(CRITERION_LARGE_FILES),
        Some(other) => Err(format!(
            "MACHINE_SETUP_BENCH_TREE_SIZE={other:?} invalid; allowed: 1000, 10000"
        )),
    }
}

/// Estimate MiB for `n_files` install pairs (`src` + `dest` PathBuf payloads).
pub fn pathbuf_list_estimate_mib(n_files: usize, avg_path_bytes: f64) -> f64 {
    let bytes = (n_files as f64) * 2.0 * avg_path_bytes;
    bytes / (1024.0 * 1024.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    Pass,
    RecommendChunk,
}

impl fmt::Display for GateVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::RecommendChunk => write!(f, "RECOMMEND_CHUNK"),
        }
    }
}

pub fn gate_verdict(peak_rss_mib: Option<f64>, pathbuf_estimate_mib: f64) -> GateVerdict {
    let rss_hit = peak_rss_mib.is_some_and(|m| m >= PEAK_RSS_GATE_MIB);
    let path_hit = pathbuf_estimate_mib >= PATHBUF_ESTIMATE_GATE_MIB;
    if rss_hit || path_hit {
        GateVerdict::RecommendChunk
    } else {
        GateVerdict::Pass
    }
}

/// Peak RSS of this process in MiB, or `None` if unsupported.
pub fn peak_rss_mib() -> Option<f64> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // SAFETY: getrusage with RUSAGE_SELF is well-defined.
        unsafe {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
            if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
                return None;
            }
            let usage = usage.assume_init();
            let raw = usage.ru_maxrss as f64;
            #[cfg(target_os = "linux")]
            {
                // Linux: KiB
                Some(raw / 1024.0)
            }
            #[cfg(target_os = "macos")]
            {
                // macOS: bytes
                Some(raw / (1024.0 * 1024.0))
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}
```

Add to `Cargo.toml` `[dependencies]`:

```toml
libc = "0.2"
```

In `src/engine/commands/mod.rs` add:

```rust
pub mod tree_measure;
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p machine_setup tree_measure -- --nocapture
```

Expected: all `tree_measure` tests PASS.

- [ ] **Step 5: Do not commit** unless the user asks.

---

### Task 2: Criterion fixture ladder

**Files:**
- Modify: `benches/command_bench.rs`

**Interfaces:**
- Consumes: `machine_setup::engine::commands::tree_measure::{parse_bench_tree_size, CRITERION_DEFAULT_FILES}`
- Produces: tree_* benches named `1k_files` or `10k_files`; `runner_smoke` always uses a dedicated 1k fixture

- [ ] **Step 1: Replace fixed `N_FILES` / `fixture_1k` with env-driven size**

Near the top of `benches/command_bench.rs`:

```rust
use machine_setup::engine::commands::tree_measure::{
    parse_bench_tree_size, CRITERION_DEFAULT_FILES,
};

fn criterion_tree_size() -> usize {
    let raw = std::env::var("MACHINE_SETUP_BENCH_TREE_SIZE").ok();
    parse_bench_tree_size(raw.as_deref()).unwrap_or_else(|e| panic!("{e}"))
}

fn size_label(n: usize) -> &'static str {
    match n {
        1_000 => "1k_files",
        10_000 => "10k_files",
        _ => panic!("unexpected criterion tree size {n}"),
    }
}

fn fixture_for(n: usize) -> &'static TreeFixture {
    // Use separate OnceLocks per allowed size so 1k runner smoke never shares
    // a 10k fixture.
    match n {
        1_000 => {
            static F: OnceLock<TreeFixture> = OnceLock::new();
            F.get_or_init(|| TreeFixture::generate(1_000))
        }
        10_000 => {
            static F: OnceLock<TreeFixture> = OnceLock::new();
            F.get_or_init(|| TreeFixture::generate(10_000))
        }
        _ => panic!("unexpected criterion tree size {n}"),
    }
}
```

Remove `const N_FILES` and old `fixture_1k()`.

- [ ] **Step 2: Wire tree_* groups to `criterion_tree_size()`**

For each of `bench_tree_install_direct`, `bench_tree_install_sudo`, `bench_mtime_skip`, `bench_symlink_tree`, `bench_uninstall_tree`:

```rust
let n = criterion_tree_size();
let fixture = fixture_for(n);
// ...
group.bench_function(size_label(n), |b| { /* unchanged body using fixture */ });
```

Keep `runner_smoke` helpers (`runner_single_copy`, `runner_parallel_two_copies`) on `fixture_for(CRITERION_DEFAULT_FILES)` only — do not call `criterion_tree_size()` there.

Update the file header comment to document `MACHINE_SETUP_BENCH_TREE_SIZE=1000|10000`.

- [ ] **Step 3: Sanity-check default bench compiles and runs briefly**

```bash
cargo bench --bench command_bench -- tree_install_direct/1k_files --warm-up-time 1 --measurement-time 1
```

Expected: completes; reports `tree_install_direct/1k_files`.

Optional local check (not CI):

```bash
MACHINE_SETUP_BENCH_TREE_SIZE=10000 cargo bench --bench command_bench -- tree_install_direct/10k_files --warm-up-time 1 --measurement-time 1
```

Expected: `10k_files` label; slower than 1k.

- [ ] **Step 4: Invalid env panics clearly**

```bash
MACHINE_SETUP_BENCH_TREE_SIZE=5000 cargo bench --bench command_bench -- tree_install_direct --warm-up-time 1 --measurement-time 1
```

Expected: panic mentioning allowed `1000, 10000`.

- [ ] **Step 5: Do not commit** unless the user asks.

---

### Task 3: `examples/tree_memory_harness.rs`

**Files:**
- Create: `examples/tree_memory_harness.rs`
- Modify: `Cargo.toml` only if an explicit `[[example]]` is required (plain `examples/*.rs` is enough for this package)

**Interfaces:**
- Consumes: `tree_measure::*`, `install_tree_with_pool` / `uninstall_tree_with_pool`, `DirectFs`, `resolve_limit`, Rayon pool
- Produces: stdout lines `n_files=…`, `peak_rss_mib=…|UNSUPPORTED`, `pathbuf_estimate_mib=…`, `verdict=…`; optional second block with `phase=uninstall`

- [ ] **Step 1: Implement the example**

Reuse the same tree layout as Criterion (`src/a/b/` + `f{i}.txt`). Sketch:

```rust
//! ADR-0004 memory gate — 100k DirectFs tree install (report-only).
//!
//! ```bash
//! cargo run --example tree_memory_harness --release
//! cargo run --example tree_memory_harness --release -- --uninstall
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rayon::ThreadPoolBuilder;
use tempfile::TempDir;

use machine_setup::engine::commands::fs_ops::DirectFs;
use machine_setup::engine::commands::tree::{install_tree_with_pool, uninstall_tree_with_pool};
use machine_setup::engine::commands::tree_measure::{
    gate_verdict, pathbuf_list_estimate_mib, peak_rss_mib, GateVerdict, MEMORY_HARNESS_FILES,
    PATHBUF_ESTIMATE_GATE_MIB, PEAK_RSS_GATE_MIB,
};
use machine_setup::engine::concurrency::resolve_limit;

fn generate_tree(root: &Path, n_files: usize) -> PathBuf {
    let src = root.join("src");
    fs::create_dir_all(src.join("a/b")).expect("mkdir");
    for i in 0..n_files {
        let sub = match i % 3 {
            0 => src.join("a/b"),
            1 => src.join("a"),
            _ => src.clone(),
        };
        fs::write(sub.join(format!("f{i}.txt")), format!("payload-{i}")).expect("write");
    }
    src
}

fn avg_path_bytes(src: &Path, dest_root: &Path, sample: usize) -> f64 {
    let mut total = 0usize;
    let mut count = 0usize;
    for i in 0..sample {
        let rel = match i % 3 {
            0 => PathBuf::from(format!("a/b/f{i}.txt")),
            1 => PathBuf::from(format!("a/f{i}.txt")),
            _ => PathBuf::from(format!("f{i}.txt")),
        };
        let s = src.join(&rel);
        let d = dest_root.join(&rel);
        total += s.as_os_str().len() + d.as_os_str().len();
        count += 2;
    }
    total as f64 / count as f64
}

fn print_block(phase: &str, n_files: usize, rss: Option<f64>, estimate: f64, verdict: GateVerdict) {
    println!("phase={phase}");
    println!("n_files={n_files}");
    match rss {
        Some(m) => println!("peak_rss_mib={m:.2}"),
        None => println!("peak_rss_mib=UNSUPPORTED"),
    }
    println!("pathbuf_estimate_mib={estimate:.2}");
    println!("gate_rss_mib={PEAK_RSS_GATE_MIB}");
    println!("gate_pathbuf_mib={PATHBUF_ESTIMATE_GATE_MIB}");
    println!("verdict={verdict}");
}

fn main() -> ExitCode {
    let want_uninstall = std::env::args().any(|a| a == "--uninstall");
    let work = TempDir::new().expect("tempdir");
    let src = generate_tree(work.path(), MEMORY_HARNESS_FILES);
    let dest = work.path().join("dest");
    fs::create_dir_all(&dest).expect("dest");

    let avg = avg_path_bytes(&src, &dest, 64);
    let estimate = pathbuf_list_estimate_mib(MEMORY_HARNESS_FILES, avg);

    let pool = ThreadPoolBuilder::new()
        .num_threads(resolve_limit(None))
        .build()
        .expect("pool");
    let ops = DirectFs;

    let rss_before = peak_rss_mib();
    if let Err(e) = install_tree_with_pool(
        &src,
        &dest,
        &[],
        Some(&pool),
        |dir| ops.mkdir_p(dir),
        |file, d| ops.copy_file(file, d),
    ) {
        eprintln!("install failed: {e}");
        return ExitCode::FAILURE;
    }
    let rss_after = peak_rss_mib();
    // Prefer post-install peak; if both present, use max.
    let rss = match (rss_before, rss_after) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => b.or(a),
    };
    let verdict = gate_verdict(rss, estimate);
    print_block("install", MEMORY_HARNESS_FILES, rss, estimate, verdict);

    if want_uninstall {
        let rss_u0 = peak_rss_mib();
        if let Err(e) = uninstall_tree_with_pool(
            &src,
            &dest,
            &[],
            Some(&pool),
            |path| {
                if path.exists() {
                    ops.remove_file(path)?;
                }
                Ok(())
            },
        ) {
            eprintln!("uninstall failed: {e}");
            return ExitCode::FAILURE;
        }
        let rss_u1 = peak_rss_mib();
        let rss_u = match (rss_u0, rss_u1) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => b.or(a),
        };
        // Uninstall list is dest-only (1 PathBuf per file); still report same
        // estimate formula for comparability, or recompute with factor 1 —
        // keep *2 formula and note in comment that ADR gate uses install phase.
        let v_u = gate_verdict(rss_u, estimate);
        print_block("uninstall", MEMORY_HARNESS_FILES, rss_u, estimate, v_u);
    }

    ExitCode::SUCCESS
}
```

Ensure `DirectFs` methods used match `benches/command_bench.rs` (`mkdir_p`, `copy_file`, `remove_file`). If `mkdir_p` is not on `DirectFs`, use the same closures as the bench (`ops.mkdir_p` / `tree::ensure_dir` patterns from `command_bench.rs`).

- [ ] **Step 2: Compile the example (debug is enough for compile; release for real numbers)**

```bash
cargo build --example tree_memory_harness
```

Expected: success.

- [ ] **Step 3: Optional local release run** (slow; generate 100k files)

```bash
cargo run --example tree_memory_harness --release
```

Expected: stdout includes `phase=install`, `n_files=100000`, `verdict=PASS` or `RECOMMEND_CHUNK`, exit 0.

- [ ] **Step 4: Do not commit** unless the user asks.

---

### Task 4: Docs (ADR-0004, CONTEXT, CHANGELOG)

**Files:**
- Modify: `docs/adr/0004-defer-parallel-tree-materialization.md`
- Modify: `CONTEXT.md` (Command bench vocabulary block ~lines 206–214)
- Modify: `CHANGELOG.md` (`[Unreleased]`)

- [ ] **Step 1: Expand ADR-0004 deferred section** to:

```markdown
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
```

- [ ] **Step 2: Extend CONTEXT.md Command bench blurb** with one sentence:

Memory harness (`examples/tree_memory_harness.rs`) is separate from Criterion:
100k DirectFs install, peak RSS + PathBuf estimate, `PASS`/`RECOMMEND_CHUNK`
against ADR-0004 thresholds; also report-only. Opt-in Criterion size via
`MACHINE_SETUP_BENCH_TREE_SIZE=10000`.

- [ ] **Step 3: CHANGELOG `[Unreleased]` Added**

```markdown
### Added
- Command-bench tree size ladder (1k default / 10k via `MACHINE_SETUP_BENCH_TREE_SIZE`) and a report-only 100k tree memory harness for the ADR-0004 chunking gate
```

- [ ] **Step 4: Final verification**

```bash
cargo test -p machine_setup tree_measure
cargo check --example tree_memory_harness
make lint
```

Expected: all green.

- [ ] **Step 5: Do not commit** unless the user asks.

---

## Spec coverage (self-review)

| Spec requirement | Task |
| --- | --- |
| Fixture ladder 1k / 10k / 100k | 2 + 3 |
| Env `MACHINE_SETUP_BENCH_TREE_SIZE` | 1 + 2 |
| Separate memory harness example | 3 |
| Peak RSS + PathBuf estimate + verdict | 1 + 3 |
| Gates 256 / 64 MiB | 1 + 4 |
| runner_smoke stays 1k | 2 |
| ADR-0004 + CONTEXT update | 4 |
| Unit tests for helpers | 1 |
| No production tree chunking | Global / no task touches apply logic |
| No CI fail on gate | Global |
| Windows RSS unsupported | 1 (`peak_rss_mib` → `None`) + 3 print |

## Placeholder scan

No TBD / “implement later” steps; concrete code and commands included.
