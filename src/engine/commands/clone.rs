use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::config::types::CloneArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

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
    async fn install(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        // Check if already cloned
        if target.join(".git").exists() {
            ctx.log(format!(
                "Repository already exists at {}, running update instead",
                target.display()
            ));
            return self.update(ctx).await;
        }

        ctx.log(format!(
            "Cloning {} into {}",
            self.args.url,
            target.display()
        ));

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        run_git_command(
            &["clone", &self.args.url, &target.to_string_lossy()],
            None,
            ctx,
        )
        .await
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if !target.join(".git").exists() {
            ctx.log("Repository not found, running install instead");
            return self.install(ctx).await;
        }

        ctx.log(format!("Pulling latest in {}", target.display()));
        run_git_command(&["pull"], Some(&target), ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        let target = expand_path(&self.args.target, Some(&ctx.config_dir));

        if target.exists() {
            ctx.log(format!("Removing repository: {}", target.display()));
            std::fs::remove_dir_all(&target)?;
        }

        Ok(())
    }

    fn description(&self) -> String {
        format!("clone: {} -> {}", self.args.url, self.args.target)
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

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::GitFailed(format!("Failed to spawn git: {e}")))?;

    let stdout_handle = child.stdout.take().map(|stdout| {
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                ctx_clone.log(line);
            }
        })
    });

    let stderr_handle = child.stderr.take().map(|stderr| {
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                ctx_clone.log(line);
            }
        })
    });

    let status = child
        .wait()
        .await
        .map_err(|e| Error::GitFailed(format!("Failed to wait for git: {e}")))?;

    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    if !status.success() {
        return Err(Error::GitFailed(format!(
            "git {} exited with code {}",
            args.join(" "),
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}
