//! ADR-0004 memory gate — 100k DirectFs tree install (report-only).
//!
//! ```bash
//! cargo run --example tree_memory_harness --release
//! cargo run --example tree_memory_harness --release -- --uninstall
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::ThreadPoolBuilder;

use machine_setup::engine::commands::fs_ops::{DirectFs, FileOps};
use machine_setup::engine::commands::tree::{install_tree_with_pool, uninstall_tree_with_pool};
use machine_setup::engine::commands::tree_measure::{
    gate_verdict, pathbuf_list_estimate_mib, peak_rss_mib, GateVerdict, MEMORY_HARNESS_FILES,
    PATHBUF_ESTIMATE_GATE_MIB, PEAK_RSS_GATE_MIB,
};
use machine_setup::engine::concurrency::resolve_limit;

struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "machine_setup_tree_memory_{}_{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("tempdir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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
    let work = WorkDir::new();
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
    let rss = match (rss_before, rss_after) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => b.or(a),
    };
    let verdict = gate_verdict(rss, estimate);
    print_block("install", MEMORY_HARNESS_FILES, rss, estimate, verdict);

    if want_uninstall {
        let rss_u0 = peak_rss_mib();
        if let Err(e) = uninstall_tree_with_pool(&src, &dest, &[], Some(&pool), |path| {
            if path.exists() {
                ops.remove_file(path)?;
            }
            Ok(())
        }) {
            eprintln!("uninstall failed: {e}");
            return ExitCode::FAILURE;
        }
        let rss_u1 = peak_rss_mib();
        let rss_u = match (rss_u0, rss_u1) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => b.or(a),
        };
        // Uninstall list is dest-only (1 PathBuf per file); keep *2 formula for
        // comparability — ADR gate uses install phase.
        let v_u = gate_verdict(rss_u, estimate);
        print_block("uninstall", MEMORY_HARNESS_FILES, rss_u, estimate, v_u);
    }

    ExitCode::SUCCESS
}
