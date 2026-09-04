//! Task event sink — the seam for emitting progress events.
//!
//! Adapters: [`ChannelSink`] (mpsc → TUI/plain) and [`NullSink`] (Command bench).

use std::sync::Arc;

use tokio::sync::mpsc;

use super::event::TaskEvent;

/// Emit Task events without callers knowing about channels or UI.
pub trait TaskEventSink: Send + Sync {
    fn emit(&self, event: TaskEvent);
}

/// Shared handle used by the Runner and CommandContext.
pub type SharedSink = Arc<dyn TaskEventSink>;

/// Forwards events onto an unbounded mpsc channel (TUI / plain logger).
pub struct ChannelSink {
    tx: mpsc::UnboundedSender<TaskEvent>,
}

impl ChannelSink {
    pub fn from_sender(tx: mpsc::UnboundedSender<TaskEvent>) -> SharedSink {
        Arc::new(Self { tx })
    }

    /// Create a channel pair wrapped as a sink + receiver.
    pub fn channel() -> (SharedSink, mpsc::UnboundedReceiver<TaskEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self::from_sender(tx), rx)
    }
}

impl TaskEventSink for ChannelSink {
    fn emit(&self, event: TaskEvent) {
        let _ = self.tx.send(event);
    }
}

/// Discards all events — used by Command bench Runner smoke.
pub struct NullSink;

impl NullSink {
    pub fn shared() -> SharedSink {
        Arc::new(Self)
    }
}

impl TaskEventSink for NullSink {
    fn emit(&self, _event: TaskEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_sink_delivers_events() {
        let (sink, mut rx) = ChannelSink::channel();
        sink.emit(TaskEvent::TaskCompleted {
            task_name: "t".into(),
        });
        match rx.try_recv() {
            Ok(TaskEvent::TaskCompleted { task_name }) => assert_eq!(task_name.as_ref(), "t"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn null_sink_drops_events() {
        let sink = NullSink::shared();
        sink.emit(TaskEvent::AllDone {
            succeeded: 1,
            failed: 0,
            skipped: 0,
        });
    }
}
