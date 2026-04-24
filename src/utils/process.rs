use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::engine::context::CommandContext;

/// Whether to tag stderr lines when forwarding them to the log.
#[derive(Copy, Clone)]
pub enum StderrLabel {
    /// Prefix stderr lines with `[stderr]` so the UI can style them.
    Prefixed,
    /// Forward stderr unchanged (useful for tools like `git` that emit
    /// progress on stderr).
    Plain,
}

/// Stream a child process's stdout and stderr to the context's event
/// channel, wait for the child to exit, and return its exit status.
pub async fn stream_and_wait(
    mut child: Child,
    ctx: &CommandContext,
    stderr_label: StderrLabel,
) -> std::io::Result<std::process::ExitStatus> {
    let stdout_handle = child.stdout.take().map(|stdout| {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                ctx.log(line);
            }
        })
    });

    let stderr_handle = child.stderr.take().map(|stderr| {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match stderr_label {
                    StderrLabel::Prefixed => ctx.log(format!("[stderr] {line}")),
                    StderrLabel::Plain => ctx.log(line),
                }
            }
        })
    });

    let status = child.wait().await?;

    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    Ok(status)
}
