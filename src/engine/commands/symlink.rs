use async_trait::async_trait;
use std::fs;
use std::path::Path;

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
                    ensure_real_dir(parent, use_sudo, ctx)?;
                }
                target.clone()
            } else {
                ensure_real_dir(&target, use_sudo, ctx)?;
                target.join(src.file_name().unwrap())
            };
            create_symlink(&src, &dest, self.args.force, use_sudo, ctx)?;
        } else {
            ensure_real_dir(&target, use_sudo, ctx)?;
            walk_relative(&src, &target, &self.args.ignore, |entry, dest| {
                if entry.file_type().is_dir() {
                    ensure_real_dir(dest, use_sudo, ctx)
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

/// Ensure `path` is a real directory — never leave a directory symlink in place.
///
/// Directory-mode symlink walks create real intermediate dirs + file symlinks.
/// A leftover directory symlink at `path` would make leaf operations resolve
/// into the source tree (self-links). Unwrap by removing only the link inode.
fn ensure_real_dir(path: &Path, use_sudo: bool, ctx: &CommandContext) -> Result<()> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            ctx.log(format!("Unwrapping directory symlink: {}", path.display()));
            remove_symlink_inode(path, use_sudo)?;
            create_dir(path, use_sudo)
        }
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(Error::PathError(format!(
            "Expected a directory at {}, found a file",
            path.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => create_dir(path, use_sudo),
        Err(err) => Err(err.into()),
    }
}

fn create_dir(path: &Path, use_sudo: bool) -> Result<()> {
    if use_sudo {
        sudo::sudo_mkdir(path)
    } else {
        fs::create_dir_all(path)?;
        Ok(())
    }
}

fn remove_symlink_inode(path: &Path, use_sudo: bool) -> Result<()> {
    if use_sudo {
        // `rm -f` removes the symlink inode; do not use `rm -rf` (would follow
        // into and delete the pointed-to tree).
        sudo::sudo_remove(path)
    } else {
        fs::remove_file(path)?;
        Ok(())
    }
}

fn create_symlink(
    src: &Path,
    dest: &Path,
    force: bool,
    use_sudo: bool,
    ctx: &CommandContext,
) -> Result<()> {
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if force {
            ctx.log(format!("Removing existing: {}", dest.display()));
            if use_sudo {
                if dest.is_dir()
                    && !dest
                        .symlink_metadata()
                        .is_ok_and(|m| m.file_type().is_symlink())
                {
                    sudo::sudo_remove_dir(dest)?;
                } else {
                    sudo::sudo_remove(dest)?;
                }
            } else if dest.is_symlink() {
                fs::remove_file(dest)?;
            } else if dest.is_dir() {
                fs::remove_dir_all(dest)?;
            } else {
                fs::remove_file(dest)?;
            }
        } else {
            ctx.log(format!("Skipping (already exists): {}", dest.display()));
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        ensure_real_dir(parent, use_sudo, ctx)?;
    }

    if let (Ok(src_canon), Ok(dest_canon)) = (fs::canonicalize(src), fs::canonicalize(dest)) {
        if src_canon == dest_canon {
            return Err(Error::PathError(format!(
                "Refusing to create self-symlink: {} -> {}",
                src.display(),
                dest.display()
            )));
        }
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

fn remove_symlink(dest: &Path, use_sudo: bool, ctx: &CommandContext) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        ctx.log(format!("Removing symlink: {}", dest.display()));
        if use_sudo {
            sudo::sudo_remove(dest)?;
        } else {
            #[cfg(windows)]
            if dest.is_dir() {
                fs::remove_dir(dest)?;
                return Ok(());
            }
            fs::remove_file(dest)?;
        }
    }
    Ok(())
}
