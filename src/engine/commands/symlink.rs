use async_trait::async_trait;
use walkdir::WalkDir;

use crate::config::types::SymlinkArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::path::{expand_path, should_ignore};

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
    async fn install(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !src.exists() {
            return Err(Error::PathError(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        if src.is_file() {
            // Source is a single file — determine destination
            let dest = if target.extension().is_some() || !target.is_dir() {
                // Target looks like a file path (e.g. /etc/wsl.conf)
                // Ensure the parent directory exists
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                target.clone()
            } else {
                // Target is a directory — place file inside it
                std::fs::create_dir_all(&target)?;
                target.join(src.file_name().unwrap())
            };
            create_symlink(&src, &dest, self.args.force, ctx)?;
        } else {
            // Source is a directory — symlink all files into target
            std::fs::create_dir_all(&target)?;
            for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                let relative = entry.path().strip_prefix(&src).unwrap();

                if should_ignore(relative, &self.args.ignore) {
                    continue;
                }

                let dest = target.join(relative);

                if entry.file_type().is_dir() {
                    std::fs::create_dir_all(&dest)?;
                } else {
                    create_symlink(entry.path(), &dest, self.args.force, ctx)?;
                }
            }
        }

        Ok(())
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        self.install(ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if src.is_file() {
            let dest = if target.extension().is_some() || !target.is_dir() {
                target
            } else {
                target.join(src.file_name().unwrap())
            };
            remove_symlink(&dest, ctx)?;
        } else {
            for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let relative = entry.path().strip_prefix(&src).unwrap();
                    if should_ignore(relative, &self.args.ignore) {
                        continue;
                    }
                    let dest = target.join(relative);
                    remove_symlink(&dest, ctx)?;
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> String {
        format!("symlink: {} -> {}", self.args.src, self.args.target)
    }
}

fn create_symlink(
    src: &std::path::Path,
    dest: &std::path::Path,
    force: bool,
    ctx: &CommandContext,
) -> Result<()> {
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if force {
            ctx.log(format!("Removing existing: {}", dest.display()));
            if dest.is_dir() {
                std::fs::remove_dir_all(dest)?;
            } else {
                std::fs::remove_file(dest)?;
            }
        } else {
            ctx.log(format!("Skipping (already exists): {}", dest.display()));
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    ctx.log(format!("Symlink: {} -> {}", src.display(), dest.display()));

    #[cfg(unix)]
    std::os::unix::fs::symlink(src, dest)?;

    #[cfg(windows)]
    {
        if src.is_dir() {
            std::os::windows::fs::symlink_dir(src, dest)?;
        } else {
            std::os::windows::fs::symlink_file(src, dest)?;
        }
    }

    Ok(())
}

fn remove_symlink(dest: &std::path::Path, ctx: &CommandContext) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        ctx.log(format!("Removing symlink: {}", dest.display()));
        #[cfg(windows)]
        if dest.is_dir() {
            std::fs::remove_dir(dest)?;
            return Ok(());
        }
        std::fs::remove_file(dest)?;
    }
    Ok(())
}
