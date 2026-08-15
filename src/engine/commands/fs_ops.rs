//! Filesystem operations behind a privilege seam.
//!
//! The `copy` and `symlink` executors need the same handful of filesystem
//! primitives, each of which has two implementations: a direct one (`std::fs`)
//! and a privileged one (shelling out to `sudo`). Rather than fork on
//! `use_sudo` at every call site — and duplicate the platform-specific symlink
//! and removal logic across both executors — the fork is decided once, when an
//! executor picks an adapter, and every primitive call goes through the
//! [`FileOps`] interface.
//!
//! Two adapters means a real seam: [`DirectFs`] and [`SudoFs`] both exist and
//! are selected at runtime by [`select`].

use std::path::Path;

use crate::error::Result;
use crate::utils::sudo;

/// The filesystem primitives `copy` and `symlink` need. Each method hides the
/// privilege decision and any platform-specific handling from callers.
pub trait FileOps: Send + Sync {
    /// Create `path` and any missing parent directories (like `mkdir -p`).
    fn mkdir_p(&self, path: &Path) -> Result<()>;

    /// Copy a single regular file from `src` to `dest`.
    ///
    /// Callers must ensure `dest`'s parent directory already exists (the
    /// Tree-op driver / Tree materialization `ensure_dir` path does this).
    fn copy_file(&self, src: &Path, dest: &Path) -> Result<()>;

    /// Create a symlink at `dest` pointing to `src`.
    fn create_symlink(&self, src: &Path, dest: &Path) -> Result<()>;

    /// Remove a regular file at `path`.
    fn remove_file(&self, path: &Path) -> Result<()>;

    /// Force-remove whatever exists at `path` — a file, or a directory and all
    /// its contents.
    fn remove_path(&self, path: &Path) -> Result<()>;

    /// Remove a symlink at `path`, handling directory symlinks on platforms
    /// (Windows) that distinguish them from file symlinks.
    fn remove_symlink(&self, path: &Path) -> Result<()>;

    /// Flush any buffered work (SudoFs script batch). DirectFs is a no-op.
    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// Pick the adapter for a command based on whether it requested `sudo`.
pub fn select(use_sudo: bool) -> Box<dyn FileOps> {
    if use_sudo {
        Box::new(SudoFs::default())
    } else {
        Box::new(DirectFs)
    }
}

/// Direct filesystem access via `std::fs`.
pub struct DirectFs;

impl FileOps for DirectFs {
    fn mkdir_p(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }

    fn copy_file(&self, src: &Path, dest: &Path) -> Result<()> {
        std::fs::copy(src, dest)?;
        Ok(())
    }

    fn create_symlink(&self, src: &Path, dest: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src, dest)?;
        }

        #[cfg(windows)]
        {
            if src.is_dir() {
                std::os::windows::fs::symlink_dir(src, dest)?;
            } else {
                std::os::windows::fs::symlink_file(src, dest)?;
            }
        }

        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)?;
        Ok(())
    }

    fn remove_path(&self, path: &Path) -> Result<()> {
        // Symlinks first: `is_dir()` follows links, and `remove_dir_all` on a
        // directory symlink would delete the pointed-to tree.
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return self.remove_symlink(path);
        }
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn remove_symlink(&self, path: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            if path.is_dir() {
                std::fs::remove_dir(path)?;
                return Ok(());
            }
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
}

/// Privileged filesystem access via `sudo`.
///
/// Ops are buffered and applied in one `sudo bash -s` on [`FileOps::flush`]
/// (ADR-0002). Callers that need an immediate single op can still use the
/// helpers in [`crate::utils::sudo`] directly (e.g. bulk `cp -a`).
pub struct SudoFs {
    pending: std::sync::Mutex<Vec<sudo::SudoOp>>,
}

impl Default for SudoFs {
    fn default() -> Self {
        Self {
            pending: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl SudoFs {
    fn push(&self, op: sudo::SudoOp) {
        self.pending.lock().expect("SudoFs lock").push(op);
    }
}

impl FileOps for SudoFs {
    fn mkdir_p(&self, path: &Path) -> Result<()> {
        self.push(sudo::SudoOp::Mkdir(path.to_path_buf()));
        Ok(())
    }

    fn copy_file(&self, src: &Path, dest: &Path) -> Result<()> {
        self.push(sudo::SudoOp::Copy {
            src: src.to_path_buf(),
            dest: dest.to_path_buf(),
        });
        Ok(())
    }

    fn create_symlink(&self, src: &Path, dest: &Path) -> Result<()> {
        self.push(sudo::SudoOp::Symlink {
            src: src.to_path_buf(),
            dest: dest.to_path_buf(),
        });
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        self.push(sudo::SudoOp::Remove(path.to_path_buf()));
        Ok(())
    }

    fn remove_path(&self, path: &Path) -> Result<()> {
        // Never `rm -rf` a directory symlink — that follows into and deletes
        // the pointed-to tree. Unlink the inode with `rm -f` instead.
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return self.remove_symlink(path);
        }
        self.push(sudo::SudoOp::RemoveDir(path.to_path_buf()));
        Ok(())
    }

    fn remove_symlink(&self, path: &Path) -> Result<()> {
        // `rm -f` on a symlink removes the link itself, file or dir target.
        self.push(sudo::SudoOp::Remove(path.to_path_buf()));
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        let ops = std::mem::take(&mut *self.pending.lock().expect("SudoFs lock"));
        if ops.is_empty() {
            return Ok(());
        }
        let script = sudo::build_sudo_script(&ops);
        sudo::sudo_bash_script(&script)
    }
}

/// An adapter that records the operations requested, without touching the real
/// filesystem — lets tests across the command executors assert *what* sequence
/// of operations an executor asked for. Test-only.
#[cfg(test)]
#[derive(Default)]
pub struct RecordingFs {
    ops: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl RecordingFs {
    /// The operations recorded so far, in order.
    pub fn calls(&self) -> Vec<String> {
        self.ops.lock().unwrap().clone()
    }
    fn record(&self, op: String) {
        self.ops.lock().unwrap().push(op);
    }
}

#[cfg(test)]
impl FileOps for RecordingFs {
    fn mkdir_p(&self, path: &Path) -> Result<()> {
        self.record(format!("mkdir_p {}", path.display()));
        Ok(())
    }
    fn copy_file(&self, src: &Path, dest: &Path) -> Result<()> {
        self.record(format!("copy_file {} {}", src.display(), dest.display()));
        Ok(())
    }
    fn create_symlink(&self, src: &Path, dest: &Path) -> Result<()> {
        self.record(format!(
            "create_symlink {} {}",
            src.display(),
            dest.display()
        ));
        Ok(())
    }
    fn remove_file(&self, path: &Path) -> Result<()> {
        self.record(format!("remove_file {}", path.display()));
        Ok(())
    }
    fn remove_path(&self, path: &Path) -> Result<()> {
        self.record(format!("remove_path {}", path.display()));
        Ok(())
    }
    fn remove_symlink(&self, path: &Path) -> Result<()> {
        self.record(format!("remove_symlink {}", path.display()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_select_returns_distinct_adapters() {
        // Smoke test: both branches construct without panicking.
        let _direct = select(false);
        let _sudo = select(true);
    }

    #[test]
    fn test_direct_mkdir_and_copy_roundtrip() {
        let dir = tempdir().unwrap();
        let ops = DirectFs;

        let src = dir.path().join("src.txt");
        std::fs::write(&src, b"hello").unwrap();

        let nested = dir.path().join("a/b/c");
        ops.mkdir_p(&nested).unwrap();
        assert!(nested.is_dir());

        // Callers ensure parents (Tree-op driver); DirectFs does not mkdir.
        let dest = dir.path().join("out/copied.txt");
        ops.mkdir_p(dest.parent().unwrap()).unwrap();
        ops.copy_file(&src, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
    }

    #[test]
    fn test_direct_remove_file_and_path() {
        let dir = tempdir().unwrap();
        let ops = DirectFs;

        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        ops.remove_file(&file).unwrap();
        assert!(!file.exists());

        let subdir = dir.path().join("tree/inner");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("leaf"), b"y").unwrap();
        ops.remove_path(&dir.path().join("tree")).unwrap();
        assert!(!dir.path().join("tree").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_direct_create_and_remove_symlink() {
        let dir = tempdir().unwrap();
        let ops = DirectFs;

        let src = dir.path().join("target.txt");
        std::fs::write(&src, b"z").unwrap();
        let link = dir.path().join("link.txt");

        ops.create_symlink(&src, &link).unwrap();
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());

        ops.remove_symlink(&link).unwrap();
        assert!(link.symlink_metadata().is_err());
        // The link target is untouched.
        assert!(src.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_direct_remove_path_unlinks_dir_symlink_without_touching_target() {
        let dir = tempdir().unwrap();
        let ops = DirectFs;

        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.txt"), b"safe").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        ops.remove_path(&link).unwrap();
        assert!(link.symlink_metadata().is_err());
        assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"safe");
    }

    #[test]
    fn test_sudofs_buffers_until_flush() {
        let ops = SudoFs::default();
        ops.mkdir_p(Path::new("/a")).unwrap();
        ops.copy_file(Path::new("/a/s"), Path::new("/a/d")).unwrap();
        let pending = ops.pending.lock().unwrap().clone();
        assert_eq!(pending.len(), 2);
        // Do not call flush() — that would invoke real sudo.
    }
}
