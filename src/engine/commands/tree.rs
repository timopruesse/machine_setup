//! Tree materialization shared by tree-shaped Command executors via the
//! **Tree-op driver**.
//!
//! Both `copy` and `symlink` face the same shape: map a source — a single file,
//! or a whole directory tree — onto a target, honoring an ignore list, and
//! either apply an operation per file (install) or undo it per file
//! (uninstall). Kind-specific per-file work and privilege policy live in the
//! Tree-op driver / `TreeOpKind`; this module owns destination resolution and
//! the walk.
//!
//! Destination resolution — the "is the target a file path or a directory to
//! drop the file into" rule — lives here as one pure, table-tested function
//! instead of being copy-pasted across four install/uninstall bodies.
//!
//! Directory installs create parents sequentially (WalkDir order), then apply
//! files on the shared ConcurrencyGate Rayon pool (ADR-0004). Small trees and
//! single-threaded pools stay sequential. No second concurrency knob.
//!
//! Large trees use **chunked apply**: during the walk, file paths accumulate
//! until the PathBuf list estimate would reach [`PATHBUF_ESTIMATE_GATE_MIB`],
//! then that chunk is applied and cleared; trees under the gate stay one chunk.

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rayon::ThreadPool;

use crate::error::{Error, Result};
use crate::utils::path::walk_relative;

use super::fs_ops::FileOps;
use super::ignore;
use super::tree_measure::{
    install_chunk_would_exceed_gate, uninstall_chunk_would_exceed_gate, DEFAULT_AVG_PATH_BYTES,
    PATHBUF_ESTIMATE_GATE_MIB,
};

/// Below this many files, thread-pool apply costs more than it saves.
const PARALLEL_FILE_THRESHOLD: usize = 32;

/// Running average of PathBuf byte lengths for chunk-size estimates.
struct PathByteAvg {
    total_bytes: usize,
    count: usize,
}

impl PathByteAvg {
    fn new() -> Self {
        Self {
            total_bytes: 0,
            count: 0,
        }
    }

    fn avg(&self) -> f64 {
        if self.count == 0 {
            DEFAULT_AVG_PATH_BYTES
        } else {
            self.total_bytes as f64 / self.count as f64
        }
    }

    fn record(&mut self, path: &Path) {
        self.total_bytes += path.as_os_str().len();
        self.count += 1;
    }

    fn record_pair(&mut self, src: &Path, dest: &Path) {
        self.record(src);
        self.record(dest);
    }

    fn clear(&mut self) {
        self.total_bytes = 0;
        self.count = 0;
    }
}

/// PathBuf estimate gate and optional flush counter (test instrumentation).
pub(crate) struct ChunkApplyConfig<'a> {
    gate_mib: f64,
    flush_count: Option<&'a std::sync::atomic::AtomicUsize>,
}

impl ChunkApplyConfig<'_> {
    fn production() -> ChunkApplyConfig<'static> {
        ChunkApplyConfig {
            gate_mib: PATHBUF_ESTIMATE_GATE_MIB,
            flush_count: None,
        }
    }

    fn note_flush(&self) {
        if let Some(counter) = self.flush_count {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Where a single source file lands, and which directory (if any) must exist
/// before it is placed there.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedDest {
    pub dest: PathBuf,
    /// Directory to create before writing `dest`, or `None` if not needed.
    pub dir_to_create: Option<PathBuf>,
}

/// Decide the destination for mapping a single source *file* onto `target`.
///
/// If `target` looks like a file path — it has an extension, or it isn't an
/// existing directory — the file lands at `target` itself and `target`'s
/// parent must exist. Otherwise `target` is an existing directory and the file
/// lands inside it under its own file name.
pub fn resolve_single_file_dest(src: &Path, target: &Path) -> ResolvedDest {
    if target.extension().is_some() || !target.is_dir() {
        ResolvedDest {
            dest: target.to_path_buf(),
            dir_to_create: target.parent().map(Path::to_path_buf),
        }
    } else {
        // Rare for real files (`/` / `..`); fall back to writing at `target`.
        match src.file_name() {
            Some(name) => ResolvedDest {
                dest: target.join(name),
                dir_to_create: Some(target.to_path_buf()),
            },
            None => ResolvedDest {
                dest: target.to_path_buf(),
                dir_to_create: target.parent().map(Path::to_path_buf),
            },
        }
    }
}

/// Apply an operation across a source onto `target` (sequential file apply).
pub fn install_tree<F, E>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    ensure_dir: E,
    on_file: F,
) -> Result<()>
where
    E: FnMut(&Path) -> Result<()>,
    F: Fn(&Path, &Path) -> Result<()> + Sync,
{
    install_tree_with_pool(src, target, ignore, None, ensure_dir, on_file)
}

/// Like [`install_tree`], applying files on `pool` when it has more than one
/// thread and the tree is large enough.
///
/// For a single file, resolves the destination, ensures the needed directory
/// via `ensure_dir`, then calls `on_file(src_file, dest)`. For a directory,
/// ensures `target` and walks the source (honoring `ignore`), ensuring
/// directories via `ensure_dir`, then applies collected files. Because WalkDir
/// yields a directory before its contents, every file's parent already exists
/// by the time `on_file` runs.
///
/// Callers choose directory semantics: `copy` passes `|d| ops.mkdir_p(d)`;
/// `symlink` passes [`ensure_real_dir`] so leftover directory symlinks cannot
/// redirect leaf operations into the source tree.
pub fn install_tree_with_pool<F, E>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    pool: Option<&ThreadPool>,
    ensure_dir: E,
    on_file: F,
) -> Result<()>
where
    E: FnMut(&Path) -> Result<()>,
    F: Fn(&Path, &Path) -> Result<()> + Sync,
{
    install_tree_with_pool_gated(
        src,
        target,
        ignore,
        pool,
        ensure_dir,
        on_file,
        ChunkApplyConfig::production(),
    )
}

/// Like [`install_tree_with_pool`], with a configurable PathBuf estimate gate
/// (MiB) for chunked apply.
pub(crate) fn install_tree_with_pool_gated<F, E>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    pool: Option<&ThreadPool>,
    mut ensure_dir: E,
    on_file: F,
    chunk: ChunkApplyConfig<'_>,
) -> Result<()>
where
    E: FnMut(&Path) -> Result<()>,
    F: Fn(&Path, &Path) -> Result<()> + Sync,
{
    if src.is_file() {
        let resolved = resolve_single_file_dest(src, target);
        if let Some(dir) = &resolved.dir_to_create {
            ensure_dir(dir)?;
        }
        return on_file(src, &resolved.dest);
    }

    ensure_dir(target)?;
    let mut files = Vec::new();
    let mut path_avg = PathByteAvg::new();
    walk_relative(
        src,
        target,
        |relative| ignore::should_ignore(relative, ignore),
        |entry, dest| {
            if entry.file_type().is_dir() {
                ensure_dir(dest)
            } else {
                let src_path = entry.path();
                if !files.is_empty()
                    && install_chunk_would_exceed_gate(
                        files.len() + 1,
                        path_avg.avg(),
                        chunk.gate_mib,
                    )
                {
                    apply_files(pool, &files, &on_file)?;
                    chunk.note_flush();
                    files.clear();
                    path_avg.clear();
                }
                path_avg.record_pair(src_path, dest);
                files.push((src_path.to_path_buf(), dest.to_path_buf()));
                Ok(())
            }
        },
    )?;

    if !files.is_empty() {
        apply_files(pool, &files, &on_file)?;
    }
    Ok(())
}

/// Ensure `path` is a real directory — never leave a directory symlink in place.
///
/// Directory-mode symlink walks create real intermediate dirs + file symlinks.
/// A leftover directory symlink at `path` would make leaf operations resolve
/// into the source tree (self-links). Unwrap by removing only the link inode.
pub fn ensure_real_dir(ops: &dyn FileOps, path: &Path, mut log: impl FnMut(String)) -> Result<()> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            log(format!("Unwrapping directory symlink: {}", path.display()));
            ops.remove_symlink(path)?;
            ops.mkdir_p(path)
        }
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(Error::PathError(format!(
            "Expected a directory at {}, found a file",
            path.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ops.mkdir_p(path),
        Err(err) => Err(err.into()),
    }
}

/// Undo an operation across a source onto `target` (sequential removes).
pub fn uninstall_tree<F>(src: &Path, target: &Path, ignore: &[String], on_dest: F) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Sync,
{
    uninstall_tree_with_pool(src, target, ignore, None, on_dest)
}

/// Like [`uninstall_tree`], removing on `pool` when parallel apply is worth it.
pub fn uninstall_tree_with_pool<F>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    pool: Option<&ThreadPool>,
    on_dest: F,
) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Sync,
{
    uninstall_tree_with_pool_gated(
        src,
        target,
        ignore,
        pool,
        on_dest,
        ChunkApplyConfig::production(),
    )
}

/// Like [`uninstall_tree_with_pool`], with a configurable PathBuf estimate gate
/// (MiB) for chunked apply.
pub(crate) fn uninstall_tree_with_pool_gated<F>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    pool: Option<&ThreadPool>,
    on_dest: F,
    chunk: ChunkApplyConfig<'_>,
) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Sync,
{
    if src.is_file() {
        let dest = resolve_single_file_dest(src, target).dest;
        return on_dest(&dest);
    }

    let mut dests = Vec::new();
    let mut path_avg = PathByteAvg::new();
    walk_relative(
        src,
        target,
        |relative| ignore::should_ignore(relative, ignore),
        |entry, dest| {
            if entry.file_type().is_file() {
                if !dests.is_empty()
                    && uninstall_chunk_would_exceed_gate(
                        dests.len() + 1,
                        path_avg.avg(),
                        chunk.gate_mib,
                    )
                {
                    apply_dests(pool, &dests, &on_dest)?;
                    chunk.note_flush();
                    dests.clear();
                    path_avg.clear();
                }
                path_avg.record(dest);
                dests.push(dest.to_path_buf());
            }
            Ok(())
        },
    )?;

    if !dests.is_empty() {
        apply_dests(pool, &dests, &on_dest)?;
    }
    Ok(())
}

fn apply_files<F>(
    pool: Option<&ThreadPool>,
    files: &[(PathBuf, PathBuf)],
    on_file: &F,
) -> Result<()>
where
    F: Fn(&Path, &Path) -> Result<()> + Sync,
{
    if let Some(pool) = pool {
        if pool.current_num_threads() > 1 && files.len() >= PARALLEL_FILE_THRESHOLD {
            return pool.install(|| {
                files
                    .par_iter()
                    .try_for_each(|(src, dest)| on_file(src, dest))
            });
        }
    }
    for (src, dest) in files {
        on_file(src, dest)?;
    }
    Ok(())
}

fn apply_dests<F>(pool: Option<&ThreadPool>, dests: &[PathBuf], on_dest: &F) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Sync,
{
    if let Some(pool) = pool {
        if pool.current_num_threads() > 1 && dests.len() >= PARALLEL_FILE_THRESHOLD {
            return pool.install(|| dests.par_iter().try_for_each(|dest| on_dest(dest)));
        }
    }
    for dest in dests {
        on_dest(dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::{DirectFs, RecordingFs};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;
    use tempfile::tempdir;

    fn test_pool(threads: usize) -> ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
    }

    #[test]
    fn test_resolve_dest_target_with_extension_is_file_path() {
        // Target has an extension → treated as the file path itself.
        let r = resolve_single_file_dest(Path::new("/src/a.txt"), Path::new("/dst/b.txt"));
        assert_eq!(r.dest, PathBuf::from("/dst/b.txt"));
        assert_eq!(r.dir_to_create, Some(PathBuf::from("/dst")));
    }

    #[test]
    fn test_resolve_dest_nonexistent_extensionless_target_is_file_path() {
        // No extension and not an existing dir → still treated as a file path.
        let r = resolve_single_file_dest(Path::new("/src/a.txt"), Path::new("/dst/newname"));
        assert_eq!(r.dest, PathBuf::from("/dst/newname"));
        assert_eq!(r.dir_to_create, Some(PathBuf::from("/dst")));
    }

    #[test]
    fn test_resolve_dest_existing_dir_target_appends_filename() {
        let dir = tempdir().unwrap();
        // An existing, extensionless directory → file drops inside it.
        let r = resolve_single_file_dest(Path::new("/src/a.txt"), dir.path());
        assert_eq!(r.dest, dir.path().join("a.txt"));
        assert_eq!(r.dir_to_create, Some(dir.path().to_path_buf()));
    }

    #[test]
    fn test_install_tree_single_file_creates_parent_then_applies() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        std::fs::write(&src, b"data").unwrap();
        let target = dir.path().join("out/file.txt");

        let ops = RecordingFs::default();
        let applied = Mutex::new(Vec::new());
        install_tree(
            &src,
            &target,
            &[],
            |d| ops.mkdir_p(d),
            |s, d| {
                applied
                    .lock()
                    .unwrap()
                    .push(format!("{} -> {}", s.display(), d.display()));
                Ok(())
            },
        )
        .unwrap();

        // Parent dir created via ops; file op invoked once with resolved dest.
        assert_eq!(
            ops.calls(),
            vec![format!("mkdir_p {}", dir.path().join("out").display())]
        );
        assert_eq!(
            applied.into_inner().unwrap(),
            vec![format!("{} -> {}", src.display(), target.display())]
        );
    }

    #[test]
    fn test_install_tree_directory_mkdirs_and_applies_per_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub/b.txt"), b"b").unwrap();
        let target = dir.path().join("dst");

        let ops = RecordingFs::default();
        let files = Mutex::new(Vec::new());
        install_tree(
            &src,
            &target,
            &[],
            |d| ops.mkdir_p(d),
            |s, _d| {
                files
                    .lock()
                    .unwrap()
                    .push(s.file_name().unwrap().to_string_lossy().into_owned());
                Ok(())
            },
        )
        .unwrap();

        let mut applied = files.into_inner().unwrap();
        applied.sort();
        assert_eq!(applied, vec!["a.txt", "b.txt"]);
        // Directories were created via ops (target + sub at least).
        let calls = ops.calls();
        assert!(calls.iter().any(|c| c.contains("dst")));
        assert!(calls.iter().any(|c| c.contains("sub")));
    }

    #[test]
    fn test_install_tree_honors_ignore() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("keep.txt"), b"k").unwrap();
        std::fs::write(src.join("README.md"), b"r").unwrap();
        let target = dir.path().join("dst");

        let ops = RecordingFs::default();
        let files = Mutex::new(Vec::new());
        install_tree(
            &src,
            &target,
            &["README.md".to_string()],
            |d| ops.mkdir_p(d),
            |s, _d| {
                files
                    .lock()
                    .unwrap()
                    .push(s.file_name().unwrap().to_string_lossy().into_owned());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(files.into_inner().unwrap(), vec!["keep.txt"]);
    }

    #[test]
    fn test_install_tree_parallel_applies_all_files() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        for i in 0..64 {
            std::fs::write(src.join(format!("f{i}.txt")), b"x").unwrap();
        }
        let target = dir.path().join("dst");
        let pool = test_pool(4);

        let applied = Mutex::new(0usize);
        install_tree_with_pool(
            &src,
            &target,
            &[],
            Some(&pool),
            |d| {
                std::fs::create_dir_all(d)?;
                Ok(())
            },
            |_s, d| {
                std::fs::write(d, b"y")?;
                *applied.lock().unwrap() += 1;
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(*applied.lock().unwrap(), 64);
        assert_eq!(std::fs::read(target.join("f0.txt")).unwrap(), b"y");
    }

    #[test]
    fn test_uninstall_tree_single_file_targets_resolved_dest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        std::fs::write(&src, b"data").unwrap();
        let target = dir.path().join("out/file.txt");

        let removed = Mutex::new(Vec::new());
        uninstall_tree(&src, &target, &[], |d| {
            removed.lock().unwrap().push(d.display().to_string());
            Ok(())
        })
        .unwrap();

        assert_eq!(
            removed.into_inner().unwrap(),
            vec![target.display().to_string()]
        );
    }

    #[test]
    fn test_uninstall_tree_directory_targets_files_only() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub/b.txt"), b"b").unwrap();
        let target = dir.path().join("dst");

        let removed = Mutex::new(Vec::new());
        uninstall_tree(&src, &target, &[], |d| {
            removed
                .lock()
                .unwrap()
                .push(d.file_name().unwrap().to_string_lossy().into_owned());
            Ok(())
        })
        .unwrap();

        let mut got = removed.into_inner().unwrap();
        got.sort();
        assert_eq!(got, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn test_install_tree_chunked_with_tiny_gate() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        for i in 0..8 {
            std::fs::write(src.join(format!("f{i}.txt")), b"x").unwrap();
        }
        let target = dir.path().join("dst");
        let flush_count = AtomicUsize::new(0);
        let applied = Mutex::new(Vec::new());

        // gate 0 forces a flush before every file after the first in-buffer entry
        install_tree_with_pool_gated(
            &src,
            &target,
            &[],
            None,
            |d| {
                std::fs::create_dir_all(d)?;
                Ok(())
            },
            |s, d| {
                std::fs::write(d, b"y")?;
                applied
                    .lock()
                    .unwrap()
                    .push(s.file_name().unwrap().to_string_lossy().into_owned());
                Ok(())
            },
            ChunkApplyConfig {
                gate_mib: 0.0,
                flush_count: Some(&flush_count),
            },
        )
        .unwrap();

        let mut names = applied.into_inner().unwrap();
        names.sort();
        assert_eq!(
            names,
            (0..8).map(|i| format!("f{i}.txt")).collect::<Vec<_>>()
        );
        assert!(
            flush_count.load(std::sync::atomic::Ordering::Relaxed) >= 2,
            "expected multiple chunk flushes, got {}",
            flush_count.load(std::sync::atomic::Ordering::Relaxed)
        );
        for i in 0..8 {
            assert_eq!(
                std::fs::read(target.join(format!("f{i}.txt"))).unwrap(),
                b"y"
            );
        }
    }

    #[test]
    fn test_uninstall_tree_chunked_with_tiny_gate() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        for i in 0..8 {
            std::fs::write(src.join(format!("f{i}.txt")), b"x").unwrap();
        }
        let target = dir.path().join("dst");
        std::fs::create_dir_all(&target).unwrap();
        for i in 0..8 {
            std::fs::write(target.join(format!("f{i}.txt")), b"y").unwrap();
        }

        let flush_count = AtomicUsize::new(0);
        let removed = Mutex::new(Vec::new());

        uninstall_tree_with_pool_gated(
            &src,
            &target,
            &[],
            None,
            |d| {
                removed
                    .lock()
                    .unwrap()
                    .push(d.file_name().unwrap().to_string_lossy().into_owned());
                std::fs::remove_file(d)?;
                Ok(())
            },
            ChunkApplyConfig {
                gate_mib: 0.0,
                flush_count: Some(&flush_count),
            },
        )
        .unwrap();

        let mut names = removed.into_inner().unwrap();
        names.sort();
        assert_eq!(
            names,
            (0..8).map(|i| format!("f{i}.txt")).collect::<Vec<_>>()
        );
        assert!(
            flush_count.load(std::sync::atomic::Ordering::Relaxed) >= 2,
            "expected multiple chunk flushes, got {}",
            flush_count.load(std::sync::atomic::Ordering::Relaxed)
        );
        for i in 0..8 {
            assert!(!target.join(format!("f{i}.txt")).exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_real_dir_unwraps_directory_symlink() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let ops = DirectFs;
        let logs = Mutex::new(Vec::new());
        ensure_real_dir(&ops, &link, |msg| logs.lock().unwrap().push(msg)).unwrap();

        let meta = link.symlink_metadata().unwrap();
        assert!(meta.is_dir() && !meta.file_type().is_symlink());
        assert!(real.exists());
        assert!(logs
            .into_inner()
            .unwrap()
            .iter()
            .any(|m| m.contains("Unwrapping directory symlink")));
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_real_dir_rejects_regular_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        let ops = DirectFs;
        let err = ensure_real_dir(&ops, &file, |_| {}).unwrap_err();
        assert!(matches!(err, Error::PathError(_)));
    }
}
