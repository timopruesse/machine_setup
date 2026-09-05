//! Command bench — Criterion micro + Runner smoke (ADR-0001 report-only).
//!
//! Generate-once fixtures per process; SudoFs cases require
//! `MACHINE_SETUP_BENCH_SUDO=1`.
//!
//! Tree ladder size: `MACHINE_SETUP_BENCH_TREE_SIZE=1000|10000|25000` (default 1000).
//! Runner smoke always uses the 1k fixture regardless of that env var.
//!
//! Fixed bring-up: `startup/task_runner_new` and `runner_smoke/empty_task`.
//! Process `--help` wall-clock is intentionally outside Criterion (manual /
//! OS process spawn — flaky under CI).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rayon::ThreadPool;
use tempfile::TempDir;

use machine_setup::config;
use machine_setup::config::types::CommandEntry;
use machine_setup::engine::commands::copy::should_skip_copy;
use machine_setup::engine::commands::create_executor;
use machine_setup::engine::commands::fs_ops::{self, DirectFs, FileOps};
use machine_setup::engine::commands::tree::{
    self, install_tree_with_pool, uninstall_tree_with_pool,
};
use machine_setup::engine::commands::tree_measure::{
    parse_bench_tree_size, CRITERION_DEFAULT_FILES,
};
use machine_setup::engine::concurrency::resolve_limit;
use machine_setup::engine::mode::Mode;
use machine_setup::engine::runner::TaskRunner;
use machine_setup::engine::sink::NullSink;

/// Mixed Command entries — deserialize + `create_executor` only (no execute).
const REGISTRY_YAML: &str = r#"
- run:
    commands: "echo hello"
- copy:
    src: /tmp/ms-bench-src
    target: /tmp/ms-bench-dst
- symlink:
    src: /tmp/ms-bench-link-src
    target: /tmp/ms-bench-link-dst
- clone:
    url: https://example.com/repo.git
    target: /tmp/ms-bench-clone
- machine_setup:
    config: /tmp/ms-bench-nested.yaml
"#;

fn criterion_tree_size() -> usize {
    let raw = std::env::var("MACHINE_SETUP_BENCH_TREE_SIZE").ok();
    parse_bench_tree_size(raw.as_deref()).unwrap_or_else(|e| panic!("{e}"))
}

fn size_label(n: usize) -> &'static str {
    match n {
        1_000 => "1k_files",
        10_000 => "10k_files",
        25_000 => "25k_files",
        _ => panic!("unexpected criterion tree size {n}"),
    }
}

struct TreeFixture {
    _root: TempDir,
    src: PathBuf,
}

impl TreeFixture {
    fn generate(n_files: usize) -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let src = root.path().join("src");
        fs::create_dir_all(src.join("a/b")).expect("mkdir");
        for i in 0..n_files {
            let sub = if i % 3 == 0 {
                src.join("a/b")
            } else if i % 3 == 1 {
                src.join("a")
            } else {
                src.clone()
            };
            fs::write(sub.join(format!("f{i}.txt")), format!("payload-{i}")).expect("write");
        }
        Self { _root: root, src }
    }

    fn src(&self) -> &Path {
        &self.src
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
        25_000 => {
            static F: OnceLock<TreeFixture> = OnceLock::new();
            F.get_or_init(|| TreeFixture::generate(25_000))
        }
        _ => panic!("unexpected criterion tree size {n}"),
    }
}

fn configure_tree_group(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    n: usize,
) {
    group.warm_up_time(Duration::from_secs(1));
    if n == 25_000 {
        group.sample_size(10);
        group.measurement_time(Duration::from_secs(10));
    } else {
        group.measurement_time(Duration::from_secs(3));
    }
}

fn bench_pool() -> &'static ThreadPool {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(resolve_limit(None))
            .build()
            .expect("bench pool")
    })
}

fn sudo_enabled() -> bool {
    std::env::var_os("MACHINE_SETUP_BENCH_SUDO").is_some_and(|v| v != "0")
}

fn copy_tree(ops: &dyn FileOps, src: &Path, dest: &Path) {
    // Match production Install short-circuit (macOS APFS `cp -cR`).
    #[cfg(target_os = "macos")]
    {
        if machine_setup::utils::fast_copy::clone_copy_tree(src, dest).is_ok() {
            return;
        }
    }
    install_tree_with_pool(
        src,
        dest,
        &[],
        Some(bench_pool()),
        |dir| ops.mkdir_p(dir),
        |file, dest| ops.copy_file(file, dest),
    )
    .expect("install_tree copy");
}

fn link_tree(ops: &dyn FileOps, src: &Path, dest: &Path) {
    // Symlink create is cheap; keep sequential stream apply (matches production).
    install_tree_with_pool(
        src,
        dest,
        &[],
        None,
        |dir| tree::ensure_real_dir(ops, dir, |_| {}),
        |file, dest| ops.create_symlink(file, dest),
    )
    .expect("install_tree symlink");
}

fn bench_tree_install_direct(c: &mut Criterion) {
    let n = criterion_tree_size();
    let fixture = fixture_for(n);
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_install_direct");
    configure_tree_group(&mut group, n);
    group.bench_function(size_label(n), |b| {
        b.iter_batched(
            || tempfile::tempdir().expect("dest"),
            |dest| {
                copy_tree(&ops, fixture.src(), dest.path());
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_tree_install_sudo(c: &mut Criterion) {
    if !sudo_enabled() {
        return;
    }
    let n = criterion_tree_size();
    let fixture = fixture_for(n);
    let ops = fs_ops::select(true);
    let mut group = c.benchmark_group("tree_install_sudo");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(if n == 25_000 {
        Duration::from_secs(10)
    } else {
        Duration::from_secs(5)
    });
    group.bench_function(size_label(n), |b| {
        b.iter_batched(
            || tempfile::tempdir().expect("dest"),
            |dest| {
                copy_tree(ops.as_ref(), fixture.src(), dest.path());
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_mtime_skip(c: &mut Criterion) {
    let n = criterion_tree_size();
    let fixture = fixture_for(n);
    let ops = DirectFs;
    let synced = tempfile::tempdir().expect("synced");
    copy_tree(&ops, fixture.src(), synced.path());

    let mut group = c.benchmark_group("mtime_skip");
    configure_tree_group(&mut group, n);
    group.bench_function(size_label(n), |b| {
        b.iter(|| {
            install_tree_with_pool(
                fixture.src(),
                synced.path(),
                &[],
                Some(bench_pool()),
                |dir| ops.mkdir_p(dir),
                |src, dest| {
                    if should_skip_copy(src, dest) {
                        return Ok(());
                    }
                    ops.copy_file(src, dest)
                },
            )
            .expect("mtime skip walk");
        });
    });
    group.finish();
}

fn bench_symlink_tree(c: &mut Criterion) {
    let n = criterion_tree_size();
    let fixture = fixture_for(n);
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_symlink_direct");
    configure_tree_group(&mut group, n);
    group.bench_function(size_label(n), |b| {
        b.iter_batched(
            || tempfile::tempdir().expect("dest"),
            |dest| {
                link_tree(&ops, fixture.src(), dest.path());
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_uninstall_tree(c: &mut Criterion) {
    let n = criterion_tree_size();
    let fixture = fixture_for(n);
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_uninstall_direct");
    configure_tree_group(&mut group, n);
    group.bench_function(size_label(n), |b| {
        b.iter_batched(
            || {
                let dest = tempfile::tempdir().expect("dest");
                copy_tree(&ops, fixture.src(), dest.path());
                dest
            },
            |dest| {
                uninstall_tree_with_pool(
                    fixture.src(),
                    dest.path(),
                    &[],
                    Some(bench_pool()),
                    |path| match ops.remove_file(path) {
                        Ok(()) => Ok(()),
                        Err(e) if matches!(&e, machine_setup::error::Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound) => {
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                )
                .expect("uninstall_tree");
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

struct RunnerCase {
    work: TempDir,
    cfg_path: PathBuf,
    cfg_dir: PathBuf,
}

fn prepare_runner_case(fixture_src: &Path, parallel: bool) -> RunnerCase {
    let work = tempfile::tempdir().expect("work");
    let cfg_dir = work.path().join("cfg");
    fs::create_dir_all(&cfg_dir).unwrap();
    let dest_a = work.path().join("out_a");
    let dest_b = work.path().join("out_b");
    let yaml = if parallel {
        format!(
            r#"
default_shell: bash
parallel: true
temp_dir: {temp}
tasks:
  copy_a:
    commands:
      - copy:
          src: "{src}"
          target: "{dest_a}"
  copy_b:
    commands:
      - copy:
          src: "{src}"
          target: "{dest_b}"
"#,
            temp = work.path().join("ms").display(),
            src = fixture_src.display(),
            dest_a = dest_a.display(),
            dest_b = dest_b.display(),
        )
    } else {
        format!(
            r#"
default_shell: bash
parallel: false
temp_dir: {temp}
tasks:
  copy_tree:
    commands:
      - copy:
          src: "{src}"
          target: "{dest}"
"#,
            temp = work.path().join("ms").display(),
            src = fixture_src.display(),
            dest = dest_a.display(),
        )
    };
    let cfg_path = cfg_dir.join("config.yaml");
    fs::write(&cfg_path, yaml).unwrap();
    RunnerCase {
        work,
        cfg_path,
        cfg_dir,
    }
}

async fn run_case(case: RunnerCase) {
    let mut config = config::load_config(case.cfg_path.to_str().unwrap()).unwrap();
    config.temp_dir = case.work.path().join("ms").to_string_lossy().into();
    let runner =
        TaskRunner::new(config, Mode::Install, NullSink::shared()).with_config_dir(case.cfg_dir);
    runner.run_all(true).await.expect("run");
}

fn prepare_empty_runner_case() -> RunnerCase {
    let work = tempfile::tempdir().expect("work");
    let cfg_dir = work.path().join("cfg");
    fs::create_dir_all(&cfg_dir).unwrap();
    // Empty `run` — no shell spawn; measures Runner / gate / history bring-up.
    let yaml = format!(
        r#"
default_shell: bash
parallel: false
temp_dir: {temp}
tasks:
  noop:
    commands:
      - run: {{}}
"#,
        temp = work.path().join("ms").display(),
    );
    let cfg_path = cfg_dir.join("config.yaml");
    fs::write(&cfg_path, yaml).unwrap();
    RunnerCase {
        work,
        cfg_path,
        cfg_dir,
    }
}

fn tiny_runner_config() -> config::types::AppConfig {
    let yaml = r#"
tasks:
  noop:
    commands:
      - run: {}
"#;
    serde_yaml::from_str(yaml).expect("tiny config")
}

fn bench_runner_smoke(c: &mut Criterion) {
    let fixture = fixture_for(CRITERION_DEFAULT_FILES);
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let mut group = c.benchmark_group("runner_smoke");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    group.bench_function("single_copy_1k_null_sink", |b| {
        b.to_async(&rt).iter_batched(
            || prepare_runner_case(fixture.src(), false),
            |case| run_case(case),
            BatchSize::LargeInput,
        );
    });

    group.bench_function("parallel_two_copy_tasks_1k", |b| {
        b.to_async(&rt).iter_batched(
            || prepare_runner_case(fixture.src(), true),
            |case| run_case(case),
            BatchSize::LargeInput,
        );
    });

    group.bench_function("empty_task", |b| {
        b.to_async(&rt).iter_batched(
            prepare_empty_runner_case,
            |case| run_case(case),
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup");
    group.sample_size(100);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    // Gate construction only — Rayon pool stays lazy until first tree apply.
    group.bench_function("task_runner_new", |b| {
        b.iter_batched(
            tiny_runner_config,
            |config| {
                let _runner = TaskRunner::new(config, Mode::Install, NullSink::shared());
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn parse_and_create_executors(yaml: &str) -> Vec<machine_setup::engine::commands::Executor> {
    let entries: Vec<CommandEntry> = serde_yaml::from_str(yaml).expect("registry yaml");
    entries.iter().map(create_executor).collect()
}

fn bench_registry(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry");
    group.sample_size(100);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("parse_and_create_executors", |b| {
        b.iter(|| parse_and_create_executors(std::hint::black_box(REGISTRY_YAML)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tree_install_direct,
    bench_tree_install_sudo,
    bench_mtime_skip,
    bench_symlink_tree,
    bench_uninstall_tree,
    bench_runner_smoke,
    bench_startup,
    bench_registry,
);
criterion_main!(benches);
