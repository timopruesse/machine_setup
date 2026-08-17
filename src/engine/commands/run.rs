use async_trait::async_trait;
use tokio::process::Command;

use crate::config::types::RunArgs;
use crate::engine::context::CommandContext;
use crate::engine::mode::Mode;
use crate::error::{Error, Result};
use crate::utils::{process, shell};

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
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        run_for_mode(&self.args, ctx.mode, ctx).await
    }

    fn description(&self) -> String {
        self.args.to_string()
    }
}

async fn run_for_mode(args: &RunArgs, mode: Mode, ctx: &CommandContext) -> Result<()> {
    let commands = args.commands_for_mode(mode);
    if commands.is_empty() {
        ctx.log_info(format!("No commands defined for mode: {mode}"));
        return Ok(());
    }

    let active_shell = args.shell.as_ref().unwrap_or(&ctx.default_shell);
    let script = shell::build_shell_command(commands, active_shell, &args.env)?;

    if !args.quiet {
        ctx.log_info(format!(
            "Running {} command(s) with {}",
            commands.len(),
            active_shell
        ));
    }

    let stream_opts = if args.quiet {
        process::StreamOptions::quiet()
    } else {
        process::StreamOptions::interactive()
    };

    let result = match active_shell {
        crate::config::types::Shell::Bash | crate::config::types::Shell::Zsh => {
            execute_script_stdin(&script, active_shell, ctx, stream_opts).await
        }
        crate::config::types::Shell::PowerShell => {
            let script_path = shell::write_temp_script(&script, active_shell, &ctx.temp_dir)?;
            let result = execute_script_file(&script_path, active_shell, ctx, stream_opts).await;
            let _ = std::fs::remove_file(&script_path);
            result
        }
    };

    if args.quiet {
        match &result {
            Ok(()) => ctx.log_info(format!("Completed {} command(s)", commands.len())),
            Err(e) => ctx.log_kind(
                crate::engine::output::OutputKind::CommandFailed,
                format!("Shell failed: {e}"),
            ),
        }
    }

    result
}

async fn execute_script_stdin(
    script: &str,
    shell_type: &crate::config::types::Shell,
    ctx: &CommandContext,
    options: process::StreamOptions,
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

    wait_with_output(child, ctx, options).await
}

async fn execute_script_file(
    script_path: &std::path::Path,
    shell_type: &crate::config::types::Shell,
    ctx: &CommandContext,
    options: process::StreamOptions,
) -> Result<()> {
    let shell_bin = shell::shell_binary(shell_type);

    let mut cmd = Command::new(shell_bin);
    cmd.arg("-File").arg(script_path);

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| Error::ShellFailed(format!("Failed to spawn {shell_bin}: {e}")))?;

    wait_with_output(child, ctx, options).await
}

async fn wait_with_output(
    child: tokio::process::Child,
    ctx: &CommandContext,
    options: process::StreamOptions,
) -> Result<()> {
    let status = process::stream_and_wait(child, ctx, options)
        .await
        .map_err(|e| Error::ShellFailed(format!("Failed to wait for shell: {e}")))?;

    if !status.success() {
        return Err(Error::ShellFailed(format!(
            "Shell exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}
