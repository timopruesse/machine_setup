//! Fast same-volume file copy helpers.
//!
//! On APFS (macOS):
//! - [`try_fast_copy`] uses `clonefile(2)` for a single-file CoW clone.
//! - [`clone_copy_tree`] uses `cp -cR` for a whole-directory clone (much faster
//!   than per-file walk+apply for Install with an empty ignore list).
//!
//! Callers must fall back to [`std::fs::copy`] / tree walk when clone is
//! unavailable (cross-device, unsupported FS, non-macOS, etc.).

use std::path::Path;

/// Try a platform fast-path copy. Returns `Ok(true)` on success, `Ok(false)`
/// when the caller should use `std::fs::copy`.
pub fn try_fast_copy(src: &Path, dest: &Path) -> std::io::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        try_clonefile(src, dest)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (src, dest);
        Ok(false)
    }
}

/// Copy `src/` contents into `target/` via macOS `cp -cR` (APFS clone when
/// possible). `target` is created if missing. On non-macOS, returns an error
/// so callers can fall back to the tree walk.
pub fn clone_copy_tree(src: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::fs::create_dir_all(target)?;
        // `src/.` copies contents into target (same shape as Tree materialization).
        let src_contents = src.join(".");
        let status = std::process::Command::new("cp")
            .arg("-cR")
            .arg(&src_contents)
            .arg(target)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "cp -cR failed with status {status}"
            )))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (src, target);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "clone_copy_tree is only available on macOS",
        ))
    }
}

#[cfg(target_os = "macos")]
fn try_clonefile(src: &Path, dest: &Path) -> std::io::Result<bool> {
    use std::cell::RefCell;
    use std::os::unix::ffi::OsStrExt;

    thread_local! {
        static SRC_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        static DEST_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    fn fill_c_path(buf: &mut Vec<u8>, path: &Path) -> bool {
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            return false;
        }
        buf.clear();
        buf.reserve(bytes.len() + 1);
        buf.extend_from_slice(bytes);
        buf.push(0);
        true
    }

    SRC_BUF.with(|src_cell| {
        DEST_BUF.with(|dest_cell| {
            let mut src_buf = src_cell.borrow_mut();
            let mut dest_buf = dest_cell.borrow_mut();
            if !fill_c_path(&mut src_buf, src) || !fill_c_path(&mut dest_buf, dest) {
                return Ok(false);
            }

            // Fresh install: one syscall. Overwrite: EEXIST → unlink → retry.
            let rc =
                unsafe { libc::clonefile(src_buf.as_ptr().cast(), dest_buf.as_ptr().cast(), 0) };
            if rc == 0 {
                return Ok(true);
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EEXIST) {
                return Ok(false);
            }
            match std::fs::remove_file(dest) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Ok(false),
            }
            let rc =
                unsafe { libc::clonefile(src_buf.as_ptr().cast(), dest_buf.as_ptr().cast(), 0) };
            Ok(rc == 0)
        })
    })
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn clonefile_copies_payload() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        std::fs::write(&src, b"hello-clone").unwrap();
        assert!(try_fast_copy(&src, &dest).unwrap());
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello-clone");
    }

    #[test]
    fn clonefile_overwrites_existing_dest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        std::fs::write(&src, b"new").unwrap();
        std::fs::write(&dest, b"old").unwrap();
        assert!(try_fast_copy(&src, &dest).unwrap());
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
    }

    #[test]
    fn clone_copy_tree_copies_contents() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub/b.txt"), b"b").unwrap();
        let dest = dir.path().join("dst");
        clone_copy_tree(&src, &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"a");
        assert_eq!(std::fs::read(dest.join("sub/b.txt")).unwrap(), b"b");
    }
}
