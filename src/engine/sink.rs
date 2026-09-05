//! Task event sink — the seam for emitting progress events.
//!
//! Adapters: [`ChannelSink`] (mpsc → TUI/plain) and [`NullSink`] (Command bench).

use std::sync::Arc;

use tokio::runtime::RuntimeFlavor;
use tokio::sync::mpsc;

use super::event::TaskEvent;

/// Bounded channel capacity for production event delivery (ADR-0005).
pub const CHANNEL_CAPACITY: usize = 8192;

/// Emit Task events without callers knowing about channels or UI.
pub trait TaskEventSink: Send + Sync {
    fn emit(&self, event: TaskEvent);
}

/// Shared handle used by the Runner and CommandContext.
pub type SharedSink = Arc<dyn TaskEventSink>;

/// Forwards events onto a bounded mpsc channel (TUI / plain logger).
pub struct ChannelSink {
    tx: mpsc::Sender<TaskEvent>,
}

impl ChannelSink {
    pub fn from_sender(tx: mpsc::Sender<TaskEvent>) -> SharedSink {
        Arc::new(Self { tx })
    }

    /// Create a channel pair wrapped as a sink + receiver.
    pub fn channel() -> (SharedSink, mpsc::Receiver<TaskEvent>) {
        Self::channel_with_capacity(CHANNEL_CAPACITY)
    }

    /// Like [`Self::channel`] with an explicit capacity (tests / benches).
    pub fn channel_with_capacity(capacity: usize) -> (SharedSink, mpsc::Receiver<TaskEvent>) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (Self::from_sender(tx), rx)
    }
}

impl TaskEventSink for ChannelSink {
    fn emit(&self, event: TaskEvent) {
        if self.tx.is_closed() {
            return;
        }
        match self.tx.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(event)) => {
                send_when_full(&self.tx, event);
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

/// Apply backpressure without panicking on a Tokio worker thread.
///
/// `Sender::blocking_send` panics inside an async runtime. On the multi-thread
/// runtime (production), park the worker via `block_in_place`. Outside a
/// runtime (e.g. sync tests, `spawn_blocking`), call `blocking_send` directly.
/// On the current-thread runtime, blocking is impossible — drop the event.
fn send_when_full(tx: &mpsc::Sender<TaskEvent>, event: TaskEvent) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::CurrentThread => {
            // Cannot block; prefer losing a progress event over panicking.
            let _ = event;
        }
        Ok(_) => {
            let tx = tx.clone();
            tokio::task::block_in_place(move || {
                let _ = tx.blocking_send(event);
            });
        }
        Err(_) => {
            let _ = tx.blocking_send(event);
        }
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn channel_sink_backpressure_on_worker_does_not_panic() {
        let (sink, mut rx) = ChannelSink::channel_with_capacity(1);

        let drained = tokio::spawn(async move {
            let first = rx.recv().await.expect("first event");
            let second = rx.recv().await.expect("second event");
            (first, second)
        });

        sink.emit(TaskEvent::TaskCompleted {
            task_name: "a".into(),
        });
        // Second emit hits Full and must block_in_place rather than panic.
        sink.emit(TaskEvent::TaskCompleted {
            task_name: "b".into(),
        });

        let (first, second) = drained.await.expect("join drain");
        match (first, second) {
            (
                TaskEvent::TaskCompleted { task_name: a },
                TaskEvent::TaskCompleted { task_name: b },
            ) => {
                assert_eq!(a.as_ref(), "a");
                assert_eq!(b.as_ref(), "b");
            }
            other => panic!("unexpected events: {other:?}"),
        }
    }
}
