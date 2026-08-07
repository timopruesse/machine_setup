use async_trait::async_trait;
use std::path::Path;

use crate::config::types::SymlinkArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::fs_ops::{self, FileOps};
use super::progress_log::FileProgress;
use super::tree;
use super::CommandExecutor;

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
        let args = self.args.clone();
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || {
            let cmd = SymlinkCommand { args };
            match ctx.mode {
                Mode::Install | Mode::Update => cmd.apply(&ctx),
                Mode::Uninstall => cmd.remove(&ctx),
            }
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

impl SymlinkCommand {
    fn apply(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !src.exists() {
            return Err(Error::PathError(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        let ops = fs_ops::select(self.args.sudo);
        let force = self.args.force;
        let progress = FileProgress::new(ctx, "symlink");
        // Symlink create is metadata-cheap; keep sequential (Command bench).
        tree::install_tree_with_pool(
            &src,
            &target,
            &self.args.ignore,
            None,
            |dir| tree::ensure_real_dir(ops.as_ref(), dir, |msg| ctx.log(msg)),
            |file, dest| symlink_one(ops.as_ref(), file, dest, force, &progress),
        )?;
        progress.finish();
        ops.flush()
    }

    fn remove(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        let ops = fs_ops::select(self.args.sudo);
        let progress = FileProgress::new(ctx, "symlink remove");
        tree::uninstall_tree_with_pool(&src, &target, &self.args.ignore, None, |dest| {
            remove_link(ops.as_ref(), dest, &progress)
        })?;
        progress.finish();
        ops.flush()
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
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if force {
            progress.note_apply(|| format!("Removing existing: {}", dest.display()));
            ops.remove_path(dest)?;
        } else {
            progress.note_skip(|| format!("Skipping (already exists): {}", dest.display()));
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        // Parent was ensured by install_tree; only unwrap leftover dir symlinks.
        tree::ensure_real_dir(ops, parent, |_| {})?;
    }

    if would_self_symlink(src, dest) {
        return Err(Error::PathError(format!(
            "Refusing to create self-symlink: {} -> {}",
            src.display(),
            dest.display()
        )));
    }

    progress.note_apply(|| format!("Symlink: {} -> {}", src.display(), dest.display()));
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
        progress.note_apply(|| format!("Removing symlink: {}", dest.display()));
        ops.remove_symlink(dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::RecordingFs;
    use tempfile::tempdir;

    fn ctx_for(
        dir: &Path,
    ) -> (
        CommandContext,
        tokio::sync::mpsc::UnboundedReceiver<crate::engine::event::TaskEvent>,
    ) {
        let (events, rx) = crate::engine::sink::ChannelSink::channel();
        let ctx = CommandContext {
            events,
            gate: std::sync::Arc::new(
                crate::engine::concurrency::ConcurrencyGate::from_num_threads(Some(1)),
            ),
            mode: Mode::Install,
            config_dir: dir.to_path_buf(),
            temp_dir: dir.to_path_buf(),
            default_shell: crate::config::types::Shell::Bash,
            task_name: "t".to_string(),
            depth: 0,
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
