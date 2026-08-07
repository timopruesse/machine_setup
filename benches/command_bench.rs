//! Command bench — Criterion micro + Runner smoke (ADR-0001 report-only).
//!
//! Generate-once fixtures per process; SudoFs cases require
//! `MACHINE_SETUP_BENCH_SUDO=1`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rayon::ThreadPool;
use tempfile::TempDir;

use machine_setup::config;
use machine_setup::engine::commands::fs_ops::{self, DirectFs, FileOps};
use machine_setup::engine::commands::tree::{
    self, install_tree_with_pool, uninstall_tree_with_pool,
};
use machine_setup::engine::concurrency::resolve_limit;
use machine_setup::engine::mode::Mode;
use machine_setup::engine::runner::TaskRunner;
use machine_setup::engine::sink::NullSink;

const N_FILES: usize = 1_000;

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

fn fixture_1k() -> &'static TreeFixture {
    static FIXTURE: OnceLock<TreeFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| TreeFixture::generate(N_FILES))
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
    // Symlink create is cheap; keep sequential (matches production SymlinkCommand).
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
    let fixture = fixture_1k();
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_install_direct");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("1k_files", |b| {
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
    let fixture = fixture_1k();
    let ops = fs_ops::select(true);
    let mut group = c.benchmark_group("tree_install_sudo");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("1k_files", |b| {
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
    let fixture = fixture_1k();
    let ops = DirectFs;
    let synced = tempfile::tempdir().expect("synced");
    copy_tree(&ops, fixture.src(), synced.path());

    let mut group = c.benchmark_group("mtime_skip");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("1k_already_synced", |b| {
        b.iter(|| {
            install_tree_with_pool(
                fixture.src(),
                synced.path(),
                &[],
                Some(bench_pool()),
                |dir| ops.mkdir_p(dir),
                |src, dest| {
                    if dest.exists() {
                        if let (Ok(sm), Ok(dm)) = (fs::metadata(src), fs::metadata(dest)) {
                            if let (Ok(smod), Ok(dmod)) = (sm.modified(), dm.modified()) {
                                if dmod >= smod {
                                    return Ok(());
                                }
                            }
                        }
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
    let fixture = fixture_1k();
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_symlink_direct");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("1k_files", |b| {
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
    let fixture = fixture_1k();
    let ops = DirectFs;
    let mut group = c.benchmark_group("tree_uninstall_direct");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("1k_files", |b| {
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
                    |path| {
                        if path.exists() {
                            ops.remove_file(path)?;
                        }
                        Ok(())
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

fn bench_runner_smoke(c: &mut Criterion) {
    let fixture = fixture_1k();
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
);
criterion_main!(benches);
