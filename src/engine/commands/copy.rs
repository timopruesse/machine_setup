use async_trait::async_trait;
use std::path::Path;

use crate::config::types::CopyArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::fs_ops::{self, FileOps};
use super::progress_log::FileProgress;
use super::tree;
use super::CommandExecutor;

pub struct CopyCommand {
    args: CopyArgs,
}

impl CopyCommand {
    pub fn new(args: CopyArgs) -> Self {
        Self { args }
    }
}

#[async_trait]
impl CommandExecutor for CopyCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        let args = self.args.clone();
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || {
            let cmd = CopyCommand { args };
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

impl CopyCommand {
    /// Directory Install with empty ignore → one `sudo cp -a` (ADR-0002).
    /// Update keeps mtime-skip via per-file + script batch.
    ///
    /// Non-sudo Install uses parallel DirectFs apply (ADR-0004) instead of a
    /// bulk `cp -a`: Command bench on WSL showed process-spawned `cp` slower
    /// than in-process parallel `std::fs::copy` for typical tree sizes.
    fn eligible_for_bulk_sudo(src: &Path, args: &CopyArgs, mode: Mode) -> bool {
        args.sudo && matches!(mode, Mode::Install) && src.is_dir() && args.ignore.is_empty()
    }

    fn apply(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !src.exists() {
            return Err(Error::PathError(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        if Self::eligible_for_bulk_sudo(&src, &self.args, ctx.mode) {
            ctx.log(format!(
                "Bulk copy (sudo): {} -> {}",
                src.display(),
                target.display()
            ));
            return crate::utils::sudo::sudo_copy_tree(&src, &target);
        }

        let ops = fs_ops::select(self.args.sudo);
        let progress = FileProgress::new(ctx, "copy");
        // SudoFs only buffers; DirectFs uses the shared gate pool (ADR-0004).
        let pool = if self.args.sudo {
            None
        } else {
            Some(ctx.gate.pool())
        };
        tree::install_tree_with_pool(
            &src,
            &target,
            &self.args.ignore,
            pool,
            |dir| ops.mkdir_p(dir),
            |file, dest| copy_one(ops.as_ref(), file, dest, &progress),
        )?;
        progress.finish();
        ops.flush()
    }

    fn remove(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        let ops = fs_ops::select(self.args.sudo);
        let progress = FileProgress::new(ctx, "copy remove");
        let pool = if self.args.sudo {
            None
        } else {
            Some(ctx.gate.pool())
        };
        tree::uninstall_tree_with_pool(&src, &target, &self.args.ignore, pool, |dest| {
            if dest.exists() {
                progress.note_apply(|| format!("Removing: {}", dest.display()));
                ops.remove_file(dest)
            } else {
                Ok(())
            }
        })?;
        progress.finish();
        ops.flush()
    }
}

/// Copy a single file, skipping when the destination is already at least as
/// new as the source.
fn copy_one(ops: &dyn FileOps, src: &Path, dest: &Path, progress: &FileProgress<'_>) -> Result<()> {
    if dest.exists() {
        if let (Ok(src_meta), Ok(dest_meta)) = (std::fs::metadata(src), std::fs::metadata(dest)) {
            if let (Ok(src_mod), Ok(dest_mod)) = (src_meta.modified(), dest_meta.modified()) {
                if dest_mod >= src_mod {
                    progress.note_skip(|| format!("Skipping (target newer): {}", dest.display()));
                    return Ok(());
                }
            }
        }
    }

    progress.note_apply(|| format!("Copying: {} -> {}", src.display(), dest.display()));
    ops.copy_file(src, dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::RecordingFs;
    use tempfile::tempdir;

    fn ctx_for(dir: &Path) -> CommandContext {
        let (events, _rx) = crate::engine::sink::ChannelSink::channel();
        CommandContext {
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
        }
    }

    #[test]
    fn test_copy_one_skips_when_dest_newer() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&src, b"old").unwrap();
        // Create dest after src so its mtime is >= src.
        std::fs::write(&dest, b"new").unwrap();

        let ctx = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "copy");
        let ops = RecordingFs::default();
        copy_one(&ops, &src, &dest, &progress).unwrap();
        // Skipped: no copy_file recorded.
        assert!(ops.calls().is_empty());
    }

    #[test]
    fn test_copy_one_copies_when_dest_missing() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&src, b"data").unwrap();

        let ctx = ctx_for(dir.path());
        let progress = FileProgress::new(&ctx, "copy");
        let ops = RecordingFs::default();
        copy_one(&ops, &src, &dest, &progress).unwrap();
        assert_eq!(
            ops.calls(),
            vec![format!("copy_file {} {}", src.display(), dest.display())]
        );
    }

    #[test]
    fn test_eligible_for_bulk_sudo() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let src_file = dir.path().join("f.txt");
        std::fs::write(&src_file, b"x").unwrap();

        let sudo_dir = CopyArgs {
            src: src_dir.to_string_lossy().into(),
            target: "/t".into(),
            ignore: vec![],
            sudo: true,
        };
        assert!(CopyCommand::eligible_for_bulk_sudo(
            &src_dir,
            &sudo_dir,
            Mode::Install
        ));
        assert!(!CopyCommand::eligible_for_bulk_sudo(
            &src_dir,
            &sudo_dir,
            Mode::Update
        ));

        let with_ignore = CopyArgs {
            ignore: vec!["x".into()],
            ..sudo_dir.clone()
        };
        assert!(!CopyCommand::eligible_for_bulk_sudo(
            &src_dir,
            &with_ignore,
            Mode::Install
        ));

        let no_sudo = CopyArgs {
            sudo: false,
            ..sudo_dir.clone()
        };
        assert!(!CopyCommand::eligible_for_bulk_sudo(
            &src_dir,
            &no_sudo,
            Mode::Install
        ));
        assert!(!CopyCommand::eligible_for_bulk_sudo(
            &src_file,
            &CopyArgs {
                src: src_file.to_string_lossy().into(),
                target: "/t".into(),
                ignore: vec![],
                sudo: true,
            },
            Mode::Install
        ));
    }
}
