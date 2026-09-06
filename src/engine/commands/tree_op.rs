//! Tree-op driver — shared Command-executor shell for tree-shaped kinds.
//!
//! Owns path expand, source existence checks, `spawn_blocking`, File ops
//! selection, progress, and shared apply_tree (walk + flush via
//! [`fs_ops::apply_tree_install`] / [`fs_ops::apply_tree_uninstall`]). Kind-specific
//! policy (bulk sudo, `force`, pool choice, per-file apply) stays behind
//! [`TreeOpKind`] (CONTEXT.md **Tree-op driver**, ADR-0002).

use std::path::Path;

use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::fs_ops::{self, FileOps};
use super::progress_log::FileProgress;

/// Per-kind policy for a tree-shaped Command executor.
pub trait TreeOpKind: Send + Sync {
    fn ignore(&self) -> &[String];
    fn sudo(&self) -> bool;
    fn progress_install(&self) -> &'static str;
    fn progress_uninstall(&self) -> &'static str;

    /// Rayon pool for install file apply; `None` = sequential.
    fn install_pool<'a>(&self, ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool>;

    /// Rayon pool for uninstall file apply; `None` = sequential.
    fn uninstall_pool<'a>(&self, ctx: &'a CommandContext) -> Option<&'a rayon::ThreadPool>;

    /// Optional short-circuit for install (e.g. bulk `sudo cp -a`). When
    /// `Some`, the normal Tree materialization walk is skipped.
    fn try_short_circuit_install(
        &self,
        src: &Path,
        target: &Path,
        ctx: &CommandContext,
    ) -> Option<Result<()>> {
        let _ = (src, target, ctx);
        None
    }

    fn ensure_dir(&self, ops: &dyn FileOps, dir: &Path, ctx: &CommandContext) -> Result<()>;

    fn on_install_file(
        &self,
        ops: &dyn FileOps,
        src: &Path,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()>;

    fn on_uninstall_file(
        &self,
        ops: &dyn FileOps,
        dest: &Path,
        progress: &FileProgress<'_>,
    ) -> Result<()>;
}

/// Run a tree-shaped Command entry: Mode dispatch on a blocking thread.
pub async fn execute(
    src: &str,
    target: &str,
    kind: impl TreeOpKind + 'static,
    ctx: &CommandContext,
) -> Result<()> {
    let src = src.to_owned();
    let target = target.to_owned();
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || run_sync(&src, &target, &kind, &ctx))
        .await
        .map_err(|e| Error::TaskJoin(e.to_string()))?
}

fn run_sync(src: &str, target: &str, kind: &dyn TreeOpKind, ctx: &CommandContext) -> Result<()> {
    let src = expand_path(src, Some(&ctx.config_dir));
    let target = expand_path(target, Some(&ctx.config_dir));
    match ctx.mode {
        Mode::Install | Mode::Update => install(&src, &target, kind, ctx),
        Mode::Uninstall => uninstall(&src, &target, kind, ctx),
    }
}

fn install(src: &Path, target: &Path, kind: &dyn TreeOpKind, ctx: &CommandContext) -> Result<()> {
    if !src.exists() {
        return Err(Error::PathError(format!(
            "Source does not exist: {}",
            src.display()
        )));
    }

    if let Some(early) = kind.try_short_circuit_install(src, target, ctx) {
        return early;
    }

    let ops = fs_ops::select_with_dry_run(kind.sudo(), ctx.dry_run);
    let progress = FileProgress::new(ctx, kind.progress_install());
    let pool = kind.install_pool(ctx);
    let _tree_apply = ctx.gate.acquire_tree_apply();
    fs_ops::apply_tree_install(
        ops.as_ref(),
        src,
        target,
        kind.ignore(),
        pool,
        |dir| kind.ensure_dir(ops.as_ref(), dir, ctx),
        |file, dest| kind.on_install_file(ops.as_ref(), file, dest, &progress),
    )?;
    progress.finish();
    Ok(())
}

fn uninstall(src: &Path, target: &Path, kind: &dyn TreeOpKind, ctx: &CommandContext) -> Result<()> {
    let ops = fs_ops::select_with_dry_run(kind.sudo(), ctx.dry_run);
    let progress = FileProgress::new(ctx, kind.progress_uninstall());
    let pool = kind.uninstall_pool(ctx);
    let _tree_apply = ctx.gate.acquire_tree_apply();
    fs_ops::apply_tree_uninstall(ops.as_ref(), src, target, kind.ignore(), pool, |dest| {
        kind.on_uninstall_file(ops.as_ref(), dest, &progress)
    })?;
    progress.finish();
    Ok(())
}
