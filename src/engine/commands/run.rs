use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::config::types::RunArgs;
use crate::engine::context::CommandContext;
use crate::error::{Error, Result};
use crate::utils::shell;

use super::CommandExecutor;

pub struct RunCommand {
    args: RunArgs,
}

impl RunCommand {
    pub fn new(args: RunArgs) -> Self {
        Self { args }
    }
}

#[async_trait]
impl CommandExecutor for RunCommand {
    async fn install(&self, ctx: &CommandContext) -> Result<()> {
        run_for_mode(&self.args, &crate::cli::Command::Install, ctx).await
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        run_for_mode(&self.args, &crate::cli::Command::Update, ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        run_for_mode(&self.args, &crate::cli::Command::Uninstall, ctx).await
    }

    fn description(&self) -> String {
        let cmds = self.args.all_command_strings();
        if cmds.is_empty() {
            "run: (no commands)".to_string()
        } else if cmds.len() == 1 {
            format!("run: {}", cmds[0])
        } else {
            format!("run: {} commands", cmds.len())
        }
    }
}

async fn run_for_mode(
    args: &RunArgs,
    mode: &crate::cli::Command,
    ctx: &CommandContext,
) -> Result<()> {
    let commands = args.commands_for_mode(mode);
    if commands.is_empty() {
        ctx.log(format!("No commands defined for mode: {mode}"));
        return Ok(());
    }

    let active_shell = args.shell.as_ref().unwrap_or(&ctx.default_shell);
    let script = shell::build_shell_command(commands, active_shell, &args.env)?;

    ctx.log(format!(
        "Running {} command(s) with {}",
        commands.len(),
        active_shell
    ));

    match active_shell {
        crate::config::types::Shell::Bash | crate::config::types::Shell::Zsh => {
            // Pipe the in-memory script straight to the shell over stdin —
            // no temp file round-trip needed.
            execute_script_stdin(&script, active_shell, ctx).await
        }
        crate::config::types::Shell::PowerShell => {
            // PowerShell needs -File for reliable execution on Windows, so
            // we still materialize a script on disk for this path only.
            let script_path = shell::write_temp_script(&script, active_shell, &ctx.temp_dir)?;
            let result = execute_script_file(&script_path, active_shell, ctx).await;
            let _ = std::fs::remove_file(&script_path);
            result
        }
    }
}

async fn execute_script_stdin(
    script: &str,
    shell_type: &crate::config::types::Shell,
    ctx: &CommandContext,
) -> Result<()> {
    let shell_bin = shell::shell_binary(shell_type);

    let mut cmd = Command::new(shell_bin);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::ShellFailed(format!("Failed to spawn {shell_bin}: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(|e| Error::ShellFailed(format!("Failed to write to stdin: {e}")))?;
        // Drop stdin to signal EOF
    }

    wait_with_output(child, ctx).await
}

async fn execute_script_file(
    script_path: &std::path::Path,
    shell_type: &crate::config::types::Shell,
    ctx: &CommandContext,
) -> Result<()> {
    let shell_bin = shell::shell_binary(shell_type);

    let mut cmd = Command::new(shell_bin);
    cmd.arg("-File").arg(script_path);

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| Error::ShellFailed(format!("Failed to spawn {shell_bin}: {e}")))?;

    wait_with_output(child, ctx).await
}

async fn wait_with_output(
    mut child: tokio::process::Child,
    ctx: &CommandContext,
) -> Result<()> {
    // Stream stdout and stderr concurrently, then wait for process
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
                ctx_clone.log(format!("[stderr] {line}"));
            }
        })
    });

    let status = child
        .wait()
        .await
        .map_err(|e| Error::ShellFailed(format!("Failed to wait for shell: {e}")))?;

    // Wait for output streams to finish flushing
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    if !status.success() {
        return Err(Error::ShellFailed(format!(
            "Shell exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}
