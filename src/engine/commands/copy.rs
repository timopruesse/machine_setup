use async_trait::async_trait;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::types::CopyArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::path::{expand_path, should_ignore};
use crate::utils::sudo;

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
        let use_sudo = self.args.sudo;

        if !src.exists() {
            return Err(Error::PathError(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        if src.is_file() {
            let dest = if target.extension().is_some() || !target.is_dir() {
                if let Some(parent) = target.parent() {
                    mkdir(parent, use_sudo)?;
                }
                target.clone()
            } else {
                mkdir(&target, use_sudo)?;
                target.join(src.file_name().unwrap())
            };
            copy_file(&src, &dest, use_sudo, ctx)?;
        } else {
            mkdir(&target, use_sudo)?;
            copy_directory(&src, &target, &self.args.ignore, use_sudo, ctx)?;
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
        let use_sudo = self.args.sudo;

        if src.is_file() {
            let dest = if target.extension().is_some() || !target.is_dir() {
                target.clone()
            } else {
                target.join(src.file_name().unwrap())
            };
            if dest.exists() {
                ctx.log(format!("Removing: {}", dest.display()));
                remove_file(&dest, use_sudo)?;
            }
        } else {
            for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let relative = entry.path().strip_prefix(&src).unwrap();
                    if should_ignore(relative, &self.args.ignore) {
                        continue;
                    }
                    let dest = target.join(relative);
                    if dest.exists() {
                        ctx.log(format!("Removing: {}", dest.display()));
                        remove_file(&dest, use_sudo)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> String {
        let prefix = if self.args.sudo {
            "copy (sudo)"
        } else {
            "copy"
        };
        format!("{prefix}: {} -> {}", self.args.src, self.args.target)
    }
}

fn mkdir(path: &Path, use_sudo: bool) -> Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    if use_sudo {
        sudo::sudo_mkdir(path)
    } else {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
}

fn remove_file(path: &Path, use_sudo: bool) -> Result<()> {
    if use_sudo {
        sudo::sudo_remove(path)
    } else {
        std::fs::remove_file(path)?;
        Ok(())
    }
}

fn copy_file(src: &Path, dest: &Path, use_sudo: bool, ctx: &CommandContext) -> Result<()> {
    // Skip if target is newer
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

    if use_sudo {
        sudo::sudo_copy(src, dest)
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest)?;
        Ok(())
    }
}

fn copy_directory(
    src: &Path,
    target: &Path,
    ignore: &[String],
    use_sudo: bool,
    ctx: &CommandContext,
) -> Result<()> {
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let relative = entry.path().strip_prefix(src).unwrap();

        if should_ignore(relative, ignore) {
            continue;
        }

        let dest = target.join(relative);

        if entry.file_type().is_dir() {
            mkdir(&dest, use_sudo)?;
        } else {
            copy_file(entry.path(), &dest, use_sudo, ctx)?;
        }
    }
    Ok(())
}
