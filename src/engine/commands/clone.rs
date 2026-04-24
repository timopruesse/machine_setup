use async_trait::async_trait;
use tokio::process::Command;

use crate::config::types::CloneArgs;
use crate::engine::context::CommandContext;
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
        self.args.to_string()
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

    // git emits progress on stderr — don't tag those lines as errors.
    let status = process::stream_and_wait(child, ctx, process::StderrLabel::Plain)
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
