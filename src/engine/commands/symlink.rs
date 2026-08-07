use async_trait::async_trait;
use std::path::Path;

use crate::config::types::SymlinkArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::fs_ops::{self, FileOps};
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
        tree::install_tree(
            &src,
            &target,
            &self.args.ignore,
            |dir| tree::ensure_real_dir(ops.as_ref(), dir, |msg| ctx.log(msg)),
            |file, dest| symlink_one(ops.as_ref(), file, dest, force, ctx),
        )?;
        ops.flush()
    }

    fn remove(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        let ops = fs_ops::select(self.args.sudo);
        tree::uninstall_tree(&src, &target, &self.args.ignore, |dest| {
            remove_link(ops.as_ref(), dest, ctx)
        })?;
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
    ctx: &CommandContext,
) -> Result<()> {
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if force {
            ctx.log(format!("Removing existing: {}", dest.display()));
            ops.remove_path(dest)?;
        } else {
            ctx.log(format!("Skipping (already exists): {}", dest.display()));
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        tree::ensure_real_dir(ops, parent, |msg| ctx.log(msg))?;
    }

    if let (Ok(src_canon), Ok(dest_canon)) =
        (std::fs::canonicalize(src), std::fs::canonicalize(dest))
    {
        if src_canon == dest_canon {
            return Err(Error::PathError(format!(
                "Refusing to create self-symlink: {} -> {}",
                src.display(),
                dest.display()
            )));
        }
    }

    ctx.log(format!("Symlink: {} -> {}", src.display(), dest.display()));
    ops.create_symlink(src, dest)
}

/// Remove the symlink an install would have created at `dest`, if present.
fn remove_link(ops: &dyn FileOps, dest: &Path, ctx: &CommandContext) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        ctx.log(format!("Removing symlink: {}", dest.display()));
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

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, false, &ctx).unwrap();
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

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, false, &ctx).unwrap();
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

        let ops = RecordingFs::default();
        symlink_one(&ops, &src, &dest, true, &ctx).unwrap();
        assert_eq!(
            ops.calls(),
            vec![
                format!("remove_path {}", dest.display()),
                format!("create_symlink {} {}", src.display(), dest.display()),
            ]
        );
    }

    #[test]
    fn test_remove_link_noop_when_absent() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("nope");
        let (ctx, _rx) = ctx_for(dir.path());

        let ops = RecordingFs::default();
        remove_link(&ops, &dest, &ctx).unwrap();
        assert!(ops.calls().is_empty());
    }
}
