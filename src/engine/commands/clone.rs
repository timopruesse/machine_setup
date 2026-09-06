use async_trait::async_trait;
use tokio::process::Command;

use crate::config::types::CloneArgs;
use crate::engine::context::{display_path, CommandContext};
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;
use crate::utils::process;

use super::CommandExecutor;

pub struct CloneCommand {
    args: CloneArgs,
}

impl CloneCommand {
    pub fn new(args: CloneArgs) -> Self {
        Self { args }
    }
}

#[async_trait]
impl CommandExecutor for CloneCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        if ctx.dry_run {
            let target = expand_path(&self.args.target, Some(&ctx.config_dir));
            match ctx.mode {
                Mode::Install => {
                    ctx.log_info(format!(
                        "[dry-run] would clone {} → {}",
                        self.args.url,
                        display_path(&target)
                    ));
                }
                Mode::Update => {
                    ctx.log_info(format!(
                        "[dry-run] would pull git repo at {}",
                        display_path(&target)
                    ));
                }
                Mode::Uninstall => {
                    ctx.log_info(format!(
                        "[dry-run] would remove git repo at {}",
                        display_path(&target)
                    ));
                }
            }
            return Ok(());
        }

        match ctx.mode {
            Mode::Install => self.clone_repo(ctx).await,
            Mode::Update => self.pull_repo(ctx).await,
            Mode::Uninstall => self.remove_repo(ctx).await,
        }
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

impl CloneCommand {
    async fn clone_repo(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if target.join(".git").exists() {
            ctx.log_progress(format!(
                "clone exists at {} — updating",
                display_path(&target)
            ));
            return self.git_pull(&target, ctx).await;
        }

        self.git_clone(&target, ctx).await
    }

    async fn pull_repo(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !target.join(".git").exists() {
            ctx.log_progress("clone missing — installing");
            return self.git_clone(&target, ctx).await;
        }

        self.git_pull(&target, ctx).await
    }

    async fn git_clone(&self, target: &std::path::Path, ctx: &CommandContext) -> Result<()> {
        ctx.log_progress(format!(
            "clone {} → {}",
            self.args.url,
            display_path(target)
        ));

        if let Some(parent) = target.parent() {
            let parent = parent.to_path_buf();
            crate::engine::host_blocking::run(move || std::fs::create_dir_all(&parent)).await??;
        }

        run_git_command(
            &[
                "clone",
                "--quiet",
                &self.args.url,
                &target.to_string_lossy(),
            ],
            None,
            ctx,
        )
        .await
    }

    async fn git_pull(&self, target: &std::path::Path, ctx: &CommandContext) -> Result<()> {
        ctx.log_progress(format!("pull {}", display_path(target)));
        run_git_command(&["pull", "--quiet"], Some(target), ctx).await
    }

    async fn remove_repo(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if target.exists() {
            ctx.log_progress(format!("remove {}", display_path(&target)));
            let target = target.to_path_buf();
            crate::engine::host_blocking::run(move || std::fs::remove_dir_all(&target)).await??;
        }

        Ok(())
    }
}

async fn run_git_command(
    args: &[&str],
    cwd: Option<&std::path::Path>,
    ctx: &CommandContext,
) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let child = cmd
        .spawn()
        .map_err(|e| Error::GitFailed(format!("Failed to spawn git: {e}")))?;

    let status = process::stream_and_wait(child, ctx, process::StreamOptions::git())
        .await
        .map_err(|e| Error::GitFailed(format!("Failed to wait for git: {e}")))?;

    if !status.success() {
        return Err(Error::GitFailed(format!(
            "git {} exited with code {}",
            args.join(" "),
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}
