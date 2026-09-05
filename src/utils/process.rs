use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::engine::context::CommandContext;
use crate::engine::output::{sanitize_subprocess_line, OutputKind};

/// Max sanitized lines per [`TaskEvent::CommandOutputBatch`] flush.
pub const OUTPUT_BATCH_SIZE: usize = 32;

/// Buffers sanitized lines until capacity or EOF flush.
#[derive(Debug, Default)]
pub struct OutputLineBuffer {
    lines: Vec<String>,
    capacity: usize,
}

impl OutputLineBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Vec::new(),
            capacity,
        }
    }

    /// Push a line; returns a full batch when capacity is reached.
    pub fn push(&mut self, line: String) -> Option<Vec<String>> {
        self.lines.push(line);
        if self.lines.len() >= self.capacity {
            Some(std::mem::take(&mut self.lines))
        } else {
            None
        }
    }

    /// Flush any remaining lines (e.g. at EOF).
    pub fn flush(&mut self) -> Option<Vec<String>> {
        if self.lines.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.lines))
        }
    }
}

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
    let mut buffer = OutputLineBuffer::new(OUTPUT_BATCH_SIZE);
    while let Ok(Some(line)) = lines.next_line().await {
        let Some(line) = sanitize_subprocess_line(line) else {
            if !last_was_blank {
                last_was_blank = true;
            }
            continue;
        };
        last_was_blank = false;
        if let Some(batch) = buffer.push(line) {
            ctx.log_batch_kind(kind, batch);
        }
    }
    if let Some(batch) = buffer.flush() {
        ctx.log_batch_kind(kind, batch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_line_buffer_flushes_at_capacity() {
        let mut buf = OutputLineBuffer::new(3);
        assert!(buf.push("a".into()).is_none());
        assert!(buf.push("b".into()).is_none());
        let batch = buf.push("c".into()).expect("full batch");
        assert_eq!(batch, vec!["a", "b", "c"]);
        assert!(buf.flush().is_none());
    }

    #[test]
    fn output_line_buffer_flush_partial() {
        let mut buf = OutputLineBuffer::new(32);
        assert!(buf.push("only".into()).is_none());
        let batch = buf.flush().expect("partial flush");
        assert_eq!(batch, vec!["only"]);
        assert!(buf.flush().is_none());
    }
}
