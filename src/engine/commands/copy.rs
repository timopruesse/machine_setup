use async_trait::async_trait;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::types::CopyArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::path::{expand_path, should_ignore};

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
    async fn install(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !src.exists() {
            return Err(Error::PathError(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        std::fs::create_dir_all(&target)?;

        if src.is_file() {
            let dest = target.join(src.file_name().unwrap());
            copy_file(&src, &dest, ctx)?;
        } else {
            copy_directory(&src, &target, &self.args.ignore, ctx)?;
        }

        Ok(())
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        ctx.log("Copy update: re-running install");
        self.install(ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if src.is_file() {
            let dest = target.join(src.file_name().unwrap());
            if dest.exists() {
                ctx.log(format!("Removing: {}", dest.display()));
                std::fs::remove_file(&dest)?;
            }
        } else {
            // Remove files that were copied from src to target
            for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let relative = entry.path().strip_prefix(&src).unwrap();
                    if should_ignore(relative, &self.args.ignore) {
                        continue;
                    }
                    let dest = target.join(relative);
                    if dest.exists() {
                        ctx.log(format!("Removing: {}", dest.display()));
                        std::fs::remove_file(&dest)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> String {
        format!("copy: {} -> {}", self.args.src, self.args.target)
    }
}

fn copy_file(src: &Path, dest: &Path, ctx: &CommandContext) -> Result<()> {
    // Skip if target is newer
    if dest.exists() {
        let src_modified = std::fs::metadata(src)?.modified()?;
        let dest_modified = std::fs::metadata(dest)?.modified()?;
        if dest_modified >= src_modified {
            ctx.log(format!("Skipping (target newer): {}", dest.display()));
            return Ok(());
        }
    }

    ctx.log(format!("Copying: {} -> {}", src.display(), dest.display()));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dest)?;
    Ok(())
}

fn copy_directory(
    src: &Path,
    target: &Path,
    ignore: &[String],
    ctx: &CommandContext,
) -> Result<()> {
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let relative = entry.path().strip_prefix(src).unwrap();

        if should_ignore(relative, ignore) {
            continue;
        }

        let dest = target.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            copy_file(entry.path(), &dest, ctx)?;
        }
    }
    Ok(())
}
