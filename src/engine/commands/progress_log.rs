//! Coalesced per-file progress logs for large tree materialization.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::engine::context::CommandContext;

/// Log the first few file ops in full, then periodically, then a summary.
const DETAIL: usize = 5;
const EVERY: usize = 100;

/// Thread-safe progress logger for copy/symlink tree walks.
pub struct FileProgress<'a> {
    ctx: &'a CommandContext,
    label: &'static str,
    applied: AtomicUsize,
    skipped: AtomicUsize,
}

impl<'a> FileProgress<'a> {
    pub fn new(ctx: &'a CommandContext, label: &'static str) -> Self {
        Self {
            ctx,
            label,
            applied: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
        }
    }

    /// Record an applied file op; may emit a detail or periodic log line.
    pub fn note_apply(&self, line: impl FnOnce() -> String) {
        let n = self.applied.fetch_add(1, Ordering::Relaxed) + 1;
        if should_log_detail(n) {
            self.ctx.log(line());
        }
    }

    /// Record a skipped file; may emit a detail or periodic log line.
    pub fn note_skip(&self, line: impl FnOnce() -> String) {
        let n = self.skipped.fetch_add(1, Ordering::Relaxed) + 1;
        if should_log_detail(n) {
            self.ctx.log(line());
        }
    }

    /// Emit a summary when the tree was large enough that detail was truncated.
    pub fn finish(&self) {
        let applied = self.applied.load(Ordering::Relaxed);
        let skipped = self.skipped.load(Ordering::Relaxed);
        if applied > DETAIL || skipped > DETAIL {
            self.ctx.log(format!(
                "{}: {} applied, {} skipped",
                self.label, applied, skipped
            ));
        }
    }
}

fn should_log_detail(n: usize) -> bool {
    n <= DETAIL || n.is_multiple_of(EVERY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_logs_first_and_every_hundred() {
        assert!(should_log_detail(1));
        assert!(should_log_detail(5));
        assert!(!should_log_detail(6));
        assert!(should_log_detail(100));
        assert!(!should_log_detail(101));
        assert!(should_log_detail(200));
    }
}
