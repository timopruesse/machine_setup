//! Tree materialization shared by the `copy` and `symlink` executors.
//!
//! Both executors face the same shape: map a source — a single file, or a
//! whole directory tree — onto a target, honoring an ignore list, and either
//! apply an operation per file (install) or undo it per file (uninstall). The
//! only genuine difference between them is what happens to one file (copy its
//! bytes vs. create a symlink), so that is the single parameter callers supply.
//!
//! Destination resolution — the "is the target a file path or a directory to
//! drop the file into" rule — lives here as one pure, table-tested function
//! instead of being copy-pasted across four install/uninstall bodies.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::utils::path::walk_relative;

use super::fs_ops::FileOps;

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
        let name = src
            .file_name()
            .expect("source file path always has a final component");
        ResolvedDest {
            dest: target.join(name),
            dir_to_create: Some(target.to_path_buf()),
        }
    }
}

/// Apply an operation across a source onto `target`.
///
/// For a single file, resolves the destination, ensures the needed directory
/// via `ensure_dir`, then calls `on_file(src_file, dest)`. For a directory,
/// ensures `target` and walks the source (honoring `ignore`), ensuring
/// directories via `ensure_dir` and calling `on_file` for each file. Because
/// WalkDir yields a directory before its contents, every file's parent already
/// exists by the time `on_file` runs.
///
/// Callers choose directory semantics: `copy` passes `|d| ops.mkdir_p(d)`;
/// `symlink` passes [`ensure_real_dir`] so leftover directory symlinks cannot
/// redirect leaf operations into the source tree.
pub fn install_tree<F, E>(
    src: &Path,
    target: &Path,
    ignore: &[String],
    mut ensure_dir: E,
    mut on_file: F,
) -> Result<()>
where
    E: FnMut(&Path) -> Result<()>,
    F: FnMut(&Path, &Path) -> Result<()>,
{
    if src.is_file() {
        let resolved = resolve_single_file_dest(src, target);
        if let Some(dir) = &resolved.dir_to_create {
            ensure_dir(dir)?;
        }
        on_file(src, &resolved.dest)?;
    } else {
        ensure_dir(target)?;
        walk_relative(src, target, ignore, |entry, dest| {
            if entry.file_type().is_dir() {
                ensure_dir(dest)
            } else {
                on_file(entry.path(), dest)
            }
        })?;
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

/// Undo an operation across a source onto `target`.
///
/// For a single file, resolves the destination and calls `on_dest(dest)`. For
/// a directory, walks the source and calls `on_dest` for each file's mapped
/// destination. Directory destinations are left in place — only the files an
/// install would have created are targeted.
pub fn uninstall_tree<F>(src: &Path, target: &Path, ignore: &[String], mut on_dest: F) -> Result<()>
where
    F: FnMut(&Path) -> Result<()>,
{
    if src.is_file() {
        let dest = resolve_single_file_dest(src, target).dest;
        on_dest(&dest)?;
    } else {
        walk_relative(src, target, ignore, |entry, dest| {
            if entry.file_type().is_file() {
                on_dest(dest)
            } else {
                Ok(())
            }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::{DirectFs, RecordingFs};
    use std::cell::RefCell;
    use tempfile::tempdir;

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
        let applied = RefCell::new(Vec::new());
        install_tree(
            &src,
            &target,
            &[],
            |d| ops.mkdir_p(d),
            |s, d| {
                applied
                    .borrow_mut()
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
            applied.into_inner(),
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
        let files = RefCell::new(Vec::new());
        install_tree(
            &src,
            &target,
            &[],
            |d| ops.mkdir_p(d),
            |s, _d| {
                files
                    .borrow_mut()
                    .push(s.file_name().unwrap().to_string_lossy().into_owned());
                Ok(())
            },
        )
        .unwrap();

        let mut applied = files.into_inner();
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
        let files = RefCell::new(Vec::new());
        install_tree(
            &src,
            &target,
            &["README.md".to_string()],
            |d| ops.mkdir_p(d),
            |s, _d| {
                files
                    .borrow_mut()
                    .push(s.file_name().unwrap().to_string_lossy().into_owned());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(files.into_inner(), vec!["keep.txt"]);
    }

    #[test]
    fn test_uninstall_tree_single_file_targets_resolved_dest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        std::fs::write(&src, b"data").unwrap();
        let target = dir.path().join("out/file.txt");

        let removed = RefCell::new(Vec::new());
        uninstall_tree(&src, &target, &[], |d| {
            removed.borrow_mut().push(d.display().to_string());
            Ok(())
        })
        .unwrap();

        assert_eq!(removed.into_inner(), vec![target.display().to_string()]);
    }

    #[test]
    fn test_uninstall_tree_directory_targets_files_only() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub/b.txt"), b"b").unwrap();
        let target = dir.path().join("dst");

        let removed = RefCell::new(Vec::new());
        uninstall_tree(&src, &target, &[], |d| {
            removed
                .borrow_mut()
                .push(d.file_name().unwrap().to_string_lossy().into_owned());
            Ok(())
        })
        .unwrap();

        let mut got = removed.into_inner();
        got.sort();
        assert_eq!(got, vec!["a.txt", "b.txt"]);
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
        let logs = RefCell::new(Vec::new());
        ensure_real_dir(&ops, &link, |msg| logs.borrow_mut().push(msg)).unwrap();

        let meta = link.symlink_metadata().unwrap();
        assert!(meta.is_dir() && !meta.file_type().is_symlink());
        assert!(real.exists());
        assert!(logs
            .into_inner()
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
