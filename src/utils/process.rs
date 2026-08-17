use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::engine::context::CommandContext;
use crate::engine::output::{sanitize_subprocess_line, OutputKind};

/// Whether to tag stderr lines when forwarding them to the log.
#[derive(Copy, Clone, Default)]
pub enum StderrLabel {
    /// Emit as [`OutputKind::SubprocessErr`].
    #[default]
    Prefixed,
    /// Emit as [`OutputKind::Subprocess`] (e.g. git progress on stderr).
    Plain,
}

/// Controls subprocess stream forwarding.
#[derive(Copy, Clone, Default)]
pub struct StreamOptions {
    /// When true, stdout is not forwarded (stderr still logged on failure paths).
    pub quiet_stdout: bool,
    pub stderr_label: StderrLabel,
}

impl StreamOptions {
    pub fn interactive() -> Self {
        Self {
            quiet_stdout: false,
            stderr_label: StderrLabel::Prefixed,
        }
    }

    pub fn quiet() -> Self {
        Self {
            quiet_stdout: true,
            stderr_label: StderrLabel::Prefixed,
        }
    }

    pub fn git() -> Self {
        Self {
            quiet_stdout: false,
            stderr_label: StderrLabel::Plain,
        }
    }
}

/// Stream a child process's stdout and stderr to the context's event
/// channel, wait for the child to exit, and return its exit status.
pub async fn stream_and_wait(
    mut child: Child,
    ctx: &CommandContext,
    options: StreamOptions,
) -> std::io::Result<std::process::ExitStatus> {
    let stdout_handle = if options.quiet_stdout {
        None
    } else {
        child.stdout.take().map(|stdout| {
            let ctx = ctx.clone();
            tokio::spawn(async move { stream_lines(stdout, &ctx, OutputKind::Subprocess).await })
        })
    };

    let stderr_label = options.stderr_label;
    let stderr_handle = child.stderr.take().map(|stderr| {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let kind = match stderr_label {
                StderrLabel::Prefixed => OutputKind::SubprocessErr,
                StderrLabel::Plain => OutputKind::Subprocess,
            };
            stream_lines(stderr, &ctx, kind).await;
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

async fn stream_lines<R>(reader: R, ctx: &CommandContext, kind: OutputKind)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut last_was_blank = false;
    while let Ok(Some(line)) = lines.next_line().await {
        let Some(line) = sanitize_subprocess_line(line) else {
            if !last_was_blank {
                last_was_blank = true;
            }
            continue;
        };
        last_was_blank = false;
        ctx.log_kind(kind, line);
    }
}
