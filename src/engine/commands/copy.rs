use async_trait::async_trait;
use std::path::Path;

use crate::config::types::CopyArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::Result;

use super::fs_ops::FileOps;
use super::progress_log::FileProgress;
use super::tree_op::{self, TreeOpKind};
use super::CommandExecutor;

#[derive(Clone)]
pub struct CopyCommand {
    args: CopyArgs,
}

impl CopyCommand {
    pub fn new(args: CopyArgs) -> Self {
        Self { args }
    }

    /// Directory Install with empty ignore → one `sudo cp -a` (ADR-0002).
    /// Update keeps mtime-skip via per-file + script batch.
    ///
    /// Non-sudo Install uses parallel DirectFs apply (ADR-0004) instead of a
    /// bulk `cp -a`: Command bench on WSL showed process-spawned `cp` slower
    /// than in-process parallel `std::fs::copy` for typical tree sizes.
    fn eligible_for_bulk_sudo(src: &Path, args: &CopyArgs, mode: Mode) -> bool {
        args.sudo && matches!(mode, Mode::Install) && src.is_dir() && args.ignore.is_empty()
    }
}

#[async_trait]
impl CommandExecutor for CopyCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        tree_op::execute(&self.args.src, &self.args.target, self.clone(), ctx).await
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

impl TreeOpKind for CopyCommand {
    fn ignore(&self) -> &[String] {
        &self.args.ignore
    }

    fn sudo(&self) -> bool {
        self.args.sudo
    }

    fn progress_install(&self) -> &'static str {
        "copy"
    }

    fn progress_uninstall(&self) -> &'static str {
        "copy remove"
    }

    fn install_pool<'a>(&self, ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool> {
        // SudoFs only buffers; DirectFs uses the shared gate pool (ADR-0004).
        if self.args.sudo {
            None
        } else {
            Some(ctx.gate.pool())
        }
    }

    fn uninstall_pool<'a>(&self, ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool> {
        if self.args.sudo {
            None
        } else {
            Some(ctx.gate.pool())
        }
    }

    fn try_short_circuit_install(
        &self,
        src: &Path,
        target: &Path,
        ctx: &CommandContext,
    ) -> Option<Result<()>> {
        if Self::eligible_for_bulk_sudo(src, &self.args, ctx.mode) {
            ctx.log_progress(format!(
                "bulk copy {} → {}",
                crate::engine::context::display_path(src),
                crate::engine::context::display_path(target),
            ));
            return Some(crate::utils::sudo::sudo_copy_tree(src, target));
        }
        None
    }

    fn ensure_dir(&self, ops: &dyn FileOps, dir: &Path, _ctx: &CommandContext) -> Result<()> {
        ops.mkdir_p(dir)
    }

    fn on_install_file(
        &self,
        ops: &dyn FileOps,
        src: &Path,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()> {
        copy_one(ops, src, dest, progress)
    }

    fn on_uninstall_file(
        &self,
        ops: &dyn FileOps,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()> {
        if dest.exists() {
            progress
                .note_apply(|| format!("remove {}", crate::engine::context::display_path(dest)));
            ops.remove_file(dest)
        } else {
            Ok(())
        }
    }
}

/// True when `dest` exists and is at least as new as `src` (mtime skip).
///
/// Uses `metadata` only — no separate `exists()` — so the already-synced
/// hot path pays fewer syscalls. Shared with Command bench.
pub fn should_skip_copy(src: &Path, dest: &Path) -> bool {
    let Ok(dest_meta) = std::fs::metadata(dest) else {
        return false;
    };
    let Ok(src_meta) = std::fs::metadata(src) else {
        return false;
    };
    match (src_meta.modified(), dest_meta.modified()) {
        (Ok(src_mod), Ok(dest_mod)) => dest_mod >= src_mod,
        _ => false,
    }
}

/// Copy a single file, skipping when the destination is already at least as
/// new as the source.
fn copy_one(ops: &dyn FileOps, src: &Path, dest: &Path, progress: &FileProgress<'_>) -> Result<()> {
    if should_skip_copy(src, dest) {
        progress.note_skip(|| {
            format!(
                "skip {} (newer)",
                crate::engine::context::display_path(dest)
            )
        });
        return Ok(());
    }

    progress.note_apply(|| {
        format!(
            "copy {} → {}",
            crate::engine::context::display_path(src),
            crate::engine::context::display_path(dest)
        )
    });
    ops.copy_file(src, dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::commands::fs_ops::RecordingFs;
    use crate::engine::mode::Mode;
    use std::sync::Arc;
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
            task_name: Arc::<str>::from("t"),
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
    fn should_skip_copy_missing_dest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        std::fs::write(&src, b"data").unwrap();
        let dest = dir.path().join("missing.txt");
        assert!(!should_skip_copy(&src, &dest));
    }

    #[test]
    fn should_skip_copy_when_dest_newer() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&src, b"old").unwrap();
        std::fs::write(&dest, b"new").unwrap();
        assert!(should_skip_copy(&src, &dest));
    }

    #[test]
    fn should_skip_copy_when_dest_older() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("s.txt");
        let dest = dir.path().join("d.txt");
        std::fs::write(&dest, b"old").unwrap();
        // Ensure src is strictly newer.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&src, b"new").unwrap();
        assert!(!should_skip_copy(&src, &dest));
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
