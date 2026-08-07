use async_trait::async_trait;
use std::path::Path;

use crate::config::types::CopyArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::fs_ops::{self, FileOps};
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
    fn eligible_for_bulk(src: &Path, args: &CopyArgs, mode: Mode) -> bool {
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

        if Self::eligible_for_bulk(&src, &self.args, ctx.mode) {
            ctx.log(format!(
                "Bulk copy (sudo): {} -> {}",
                src.display(),
                target.display()
            ));
            return crate::utils::sudo::sudo_copy_tree(&src, &target);
        }

        let ops = fs_ops::select(self.args.sudo);
        tree::install_tree(
            &src,
            &target,
            &self.args.ignore,
            |dir| ops.mkdir_p(dir),
            |file, dest| copy_one(ops.as_ref(), file, dest, ctx),
        )?;
        ops.flush()
    }

    fn remove(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        let ops = fs_ops::select(self.args.sudo);
        tree::uninstall_tree(&src, &target, &self.args.ignore, |dest| {
            if dest.exists() {
                ctx.log(format!("Removing: {}", dest.display()));
                ops.remove_file(dest)
            } else {
                Ok(())
            }
        })?;
        ops.flush()
    }
}

/// Copy a single file, skipping when the destination is already at least as
/// new as the source.
fn copy_one(ops: &dyn FileOps, src: &Path, dest: &Path, ctx: &CommandContext) -> Result<()> {
    if dest.exists() {
        if let (Ok(src_meta), Ok(dest_meta)) = (std::fs::metadata(src), std::fs::metadata(dest)) {
            if let (Ok(src_mod), Ok(dest_mod)) = (src_meta.modified(), dest_meta.modified()) {
                if dest_mod >= src_mod {
                    ctx.log(format!("Skipping (target newer): {}", dest.display()));
                    return Ok(());
                }
            }
        }
    }

    ctx.log(format!("Copying: {} -> {}", src.display(), dest.display()));
    ops.copy_file(src, dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::RecordingFs;
    use tempfile::tempdir;

    #[test]
    fn test_copy_one_skips_when_dest_newer() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&src, b"old").unwrap();
        // Create dest after src so its mtime is >= src.
        std::fs::write(&dest, b"new").unwrap();

        let (events, _rx) = crate::engine::sink::ChannelSink::channel();
        let ctx = CommandContext {
            events,
            gate: std::sync::Arc::new(
                crate::engine::concurrency::ConcurrencyGate::from_num_threads(Some(1)),
            ),
            mode: Mode::Install,
            config_dir: dir.path().to_path_buf(),
            temp_dir: dir.path().to_path_buf(),
            default_shell: crate::config::types::Shell::Bash,
            task_name: "t".to_string(),
            depth: 0,
        };

        let ops = RecordingFs::default();
        copy_one(&ops, &src, &dest, &ctx).unwrap();
        // Skipped: no copy_file recorded.
        assert!(ops.calls().is_empty());
    }

    #[test]
    fn test_copy_one_copies_when_dest_missing() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&src, b"data").unwrap();

        let (events, _rx) = crate::engine::sink::ChannelSink::channel();
        let ctx = CommandContext {
            events,
            gate: std::sync::Arc::new(
                crate::engine::concurrency::ConcurrencyGate::from_num_threads(Some(1)),
            ),
            mode: Mode::Install,
            config_dir: dir.path().to_path_buf(),
            temp_dir: dir.path().to_path_buf(),
            default_shell: crate::config::types::Shell::Bash,
            task_name: "t".to_string(),
            depth: 0,
        };

        let ops = RecordingFs::default();
        copy_one(&ops, &src, &dest, &ctx).unwrap();
        assert_eq!(
            ops.calls(),
            vec![format!("copy_file {} {}", src.display(), dest.display())]
        );
    }

    #[test]
    fn test_eligible_for_bulk_copy() {
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
        assert!(CopyCommand::eligible_for_bulk(
            &src_dir,
            &sudo_dir,
            Mode::Install
        ));
        assert!(!CopyCommand::eligible_for_bulk(
            &src_dir,
            &sudo_dir,
            Mode::Update
        ));

        let with_ignore = CopyArgs {
            ignore: vec!["x".into()],
            ..sudo_dir.clone()
        };
        assert!(!CopyCommand::eligible_for_bulk(
            &src_dir,
            &with_ignore,
            Mode::Install
        ));

        let no_sudo = CopyArgs {
            sudo: false,
            ..sudo_dir.clone()
        };
        assert!(!CopyCommand::eligible_for_bulk(
            &src_dir,
            &no_sudo,
            Mode::Install
        ));
        assert!(!CopyCommand::eligible_for_bulk(
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
