/// Classification for log lines crossing the Task event sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputKind {
    /// Command entry started (replaces `> desc` prefix).
    CommandStart,
    /// Command entry finished successfully.
    CommandDone,
    /// Command entry failed (detail line; task-level failure is separate).
    CommandFailed,
    /// Structured progress from copy/symlink/clone/setup executors.
    Progress,
    /// Subprocess stdout or unlabeled stderr (e.g. git progress).
    #[default]
    Subprocess,
    /// Subprocess stderr from shell `run` commands.
    SubprocessErr,
    /// Informational executor message (run preamble, empty-mode notice).
    Info,
    /// Task lifecycle (start, complete, skip, retry).
    TaskStatus,
}

impl OutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandStart => "command_start",
            Self::CommandDone => "command_done",
            Self::CommandFailed => "command_failed",
            Self::Progress => "progress",
            Self::Subprocess => "subprocess",
            Self::SubprocessErr => "subprocess_err",
            Self::Info => "info",
            Self::TaskStatus => "task_status",
        }
    }
}

/// Max visible width for subprocess lines before truncation.
pub const MAX_SUBPROCESS_LINE_LEN: usize = 500;

/// Sanitize a subprocess line: drop empty/whitespace-only, truncate long lines.
pub fn sanitize_subprocess_line(line: String) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= MAX_SUBPROCESS_LINE_LEN {
        return Some(trimmed.to_string());
    }
    let take = MAX_SUBPROCESS_LINE_LEN.saturating_sub(1);
    let mut out: String = trimmed.chars().take(take).collect();
    out.push('…');
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_blank_lines() {
        assert_eq!(sanitize_subprocess_line("   ".into()), None);
    }

    #[test]
    fn truncates_long_lines() {
        let long = "a".repeat(MAX_SUBPROCESS_LINE_LEN + 10);
        let out = sanitize_subprocess_line(long).unwrap();
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= MAX_SUBPROCESS_LINE_LEN);
    }
}
