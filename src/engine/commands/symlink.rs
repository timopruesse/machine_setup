use async_trait::async_trait;

use crate::config::types::SymlinkArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::path::{expand_path, walk_relative};
use crate::utils::sudo;

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
            create_symlink(&src, &dest, self.args.force, use_sudo, ctx)?;
        } else {
            mkdir(&target, use_sudo)?;
            walk_relative(&src, &target, &self.args.ignore, |entry, dest| {
                if entry.file_type().is_dir() {
                    mkdir(dest, use_sudo)
                } else {
                    create_symlink(entry.path(), dest, self.args.force, use_sudo, ctx)
                }
            })?;
        }

        Ok(())
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        self.install(ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        let src = expand_path(&self.args.src, Some(&ctx.config_dir));
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));
        let use_sudo = self.args.sudo;

        if src.is_file() {
            let dest = if target.extension().is_some() || !target.is_dir() {
                target
            } else {
                target.join(src.file_name().unwrap())
            };
            remove_symlink(&dest, use_sudo, ctx)?;
        } else {
            walk_relative(&src, &target, &self.args.ignore, |entry, dest| {
                if entry.file_type().is_file() {
                    remove_symlink(dest, use_sudo, ctx)?;
                }
                Ok(())
            })?;
        }

        Ok(())
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

fn mkdir(path: &std::path::Path, use_sudo: bool) -> Result<()> {
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

fn create_symlink(
    src: &std::path::Path,
    dest: &std::path::Path,
    force: bool,
    use_sudo: bool,
    ctx: &CommandContext,
) -> Result<()> {
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if force {
            ctx.log(format!("Removing existing: {}", dest.display()));
            if use_sudo {
                if dest.is_dir() {
                    sudo::sudo_remove_dir(dest)?;
                } else {
                    sudo::sudo_remove(dest)?;
                }
            } else if dest.is_dir() {
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
        mkdir(parent, use_sudo)?;
    }

    ctx.log(format!("Symlink: {} -> {}", src.display(), dest.display()));

    if use_sudo {
        sudo::sudo_symlink(src, dest)
    } else {
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
}

fn remove_symlink(dest: &std::path::Path, use_sudo: bool, ctx: &CommandContext) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        ctx.log(format!("Removing symlink: {}", dest.display()));
        if use_sudo {
            sudo::sudo_remove(dest)?;
        } else {
            #[cfg(windows)]
            if dest.is_dir() {
                std::fs::remove_dir(dest)?;
                return Ok(());
            }
            std::fs::remove_file(dest)?;
        }
    }
    Ok(())
}
