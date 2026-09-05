use async_trait::async_trait;
use std::path::Path;

use crate::config::types::SymlinkArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};

use super::fs_ops::FileOps;
use super::progress_log::FileProgress;
use super::tree;
use super::tree_op::{self, TreeOpKind};
use super::CommandExecutor;

#[derive(Clone)]
pub struct SymlinkCommand {
    args: SymlinkArgs,
}

impl SymlinkCommand {
    pub fn new(args: SymlinkArgs) -> Self {
        Self { args }
    }
}

#[async_trait]
impl CommandExecutor for SymlinkCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        tree_op::execute(&self.args.src, &self.args.target, self.clone(), ctx).await
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

impl TreeOpKind for SymlinkCommand {
    fn ignore(&self) -> &[String] {
        &self.args.ignore
    }

    fn sudo(&self) -> bool {
        self.args.sudo
    }

    fn progress_install(&self) -> &'static str {
        "symlink"
    }

    fn progress_uninstall(&self) -> &'static str {
        "symlink remove"
    }

    fn install_pool<'a>(&self, _ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool> {
        // Symlink create is metadata-cheap; parallel collect+apply is slower
        // than sequential stream apply (Command bench).
        None
    }

    fn uninstall_pool<'a>(&self, _ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool> {
        None
    }

    fn ensure_dir(&self, ops: &dyn FileOps, dir: &Path, ctx: &CommandContext) -> Result<()> {
        tree::ensure_real_dir(ops, dir, |msg| ctx.log_progress(msg))
    }

    fn on_install_file(
        &self,
        ops: &dyn FileOps,
        src: &Path,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()> {
        symlink_one(ops, src, dest, self.args.force, progress)
    }

    fn on_uninstall_file(
        &self,
        ops: &dyn FileOps,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()> {
        remove_link(ops, dest, progress)
    }
}

/// Create one symlink at `dest` pointing to `src`. When something already
/// exists at `dest`, either replace it (`force`) or skip it.
fn symlink_one(
    ops: &dyn FileOps,
    src: &Path,
    dest: &Path,
    force: bool,
    progress: &FileProgress<'_>,
) -> Result<()> {
    // One metadata syscall covers files, dirs, and broken symlinks (avoid
    // `exists()` + `symlink_metadata()` on the common "absent" create path).
    if dest.symlink_metadata().is_ok() {
        if force {
            progress
                .note_apply(|| format!("remove {}", crate::engine::context::display_path(dest)));
            ops.remove_path(dest)?;
        } else {
            progress.note_skip(|| {
                format!(
                    "skip {} (exists)",
                    crate::engine::context::display_path(dest)
                )
            });
            return Ok(());
        }
    }

    if would_self_symlink(src, dest) {
        return Err(Error::PathError(format!(
            "Refusing to create self-symlink: {} -> {}",
            src.display(),
            dest.display()
        )));
    }

    progress.note_apply(|| {
        format!(
            "link {} → {}",
            crate::engine::context::display_path(src),
            crate::engine::context::display_path(dest)
        )
    });
    ops.create_symlink(src, dest)
}

/// Cheap self-link check: path equality first; canonicalize only when both exist.
fn would_self_symlink(src: &Path, dest: &Path) -> bool {
    if src == dest {
        return true;
    }
    // Dest usually does not exist yet — skip canonicalize syscalls in that case.
    if dest.symlink_metadata().is_err() {
        return false;
    }
    match (src.canonicalize(), dest.canonicalize()) {
        (Ok(src_canon), Ok(dest_canon)) => src_canon == dest_canon,
        _ => false,
    }
}

/// Remove the symlink an install would have created at `dest`, if present.
fn remove_link(ops: &dyn FileOps, dest: &Path, progress: &FileProgress<'_>) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        progress.note_apply(|| format!("unlink {}", crate::engine::context::display_path(dest)));
        ops.remove_symlink(dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::RecordingFs;
    use crate::engine::mode::Mode;
    use tempfile::tempdir;

    fn ctx_for(
        dir: &Path,
    ) -> (
        CommandContext,
        tokio::sync::mpsc::Receiver<crate::engine::event::TaskEvent>,
    ) {
        let (events, rx) = crate::engine::sink::ChannelSink::channel();
        let ctx = CommandContext {
            events,
            gate: std::sync::Arc::new(
                crate::engine::concurrency::ConcurrencyGate::from_num_threads(Some(1)),
            ),
            mode: Mode::Install,
            config_dir: std::sync::Arc::new(dir.to_path_buf()),
            temp_dir: std::sync::Arc::new(dir.to_path_buf()),
            default_shell: crate::config::types::Shell::Bash,
            task_name: std::sync::Arc::<str>::from("t"),
            depth: 0,
            cancel: tokio_util::sync::CancellationToken::new(),
        };
        (ctx, rx)
    }

    #[test]
    fn test_symlink_one_creates_when_absent() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("link");
        let (ctx, _rx) = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "symlink");

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, false, &progress).unwrap();
        assert_eq!(
            ops.calls(),
            vec![format!(
                "create_symlink {} {}",
                src.display(),
                dest.display()
            )]
        );
    }

    #[test]
    fn test_symlink_one_skips_existing_without_force() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("existing");
        std::fs::write(&dest, b"old").unwrap();
        let (ctx, _rx) = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "symlink");

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, false, &progress).unwrap();
        // Skipped: nothing touched.
        assert!(ops.calls().is_empty());
    }

    #[test]
    fn test_symlink_one_force_removes_then_creates() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("existing");
        std::fs::write(&dest, b"old").unwrap();
        let (ctx, _rx) = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "symlink");

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, true, &progress).unwrap();
        assert_eq!(
            ops.calls(),
            vec![
                format!("remove_path {}", dest.display()),
                format!("create_symlink {} {}", src.display(), dest.display()),
            ]
        );
    }

    #[test]
    fn test_would_self_symlink_path_equality() {
        let p = Path::new("/tmp/x");
        assert!(would_self_symlink(p, p));
    }

    #[test]
    fn test_would_self_symlink_skips_canonicalize_when_dest_missing() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("missing-link");
        assert!(!would_self_symlink(&src, &dest));
    }

    #[test]
    fn test_remove_link_noop_when_absent() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("nope");
        let (ctx, _rx) = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "symlink remove");

        let ops = RecordingFs::default();
        remove_link(&ops, &dest, &progress).unwrap();
        assert!(ops.calls().is_empty());
    }
}
