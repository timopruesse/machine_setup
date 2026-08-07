//! Concurrency gate — global cap on in-flight Task / Command executor work.
//!
//! See ADR-0003. Does not order Tasks by dependency (that is the Task graph).

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Shared limit on concurrent Task and Command executor work.
#[derive(Clone)]
pub struct ConcurrencyGate {
    sem: Arc<Semaphore>,
    /// Configured permit count (for tests / diagnostics).
    pub limit: usize,
}

impl ConcurrencyGate {
    /// Build a gate from `AppConfig.num_threads` (None → CPUs − 1, at least 1).
    pub fn from_num_threads(num_threads: Option<usize>) -> Self {
        let limit = resolve_limit(num_threads);
        Self {
            sem: Arc::new(Semaphore::new(limit)),
            limit,
        }
    }

    /// Acquire one permit; held for the lifetime of the returned guard.
    pub async fn acquire(self: &Arc<Self>) -> OwnedSemaphorePermit {
        self.sem
            .clone()
            .acquire_owned()
            .await
            .expect("ConcurrencyGate semaphore is never closed")
    }
}

/// Resolve `num_threads` to a positive permit count.
///
/// `None` → `available_parallelism() - 1` (minimum 1), matching the README default.
pub fn resolve_limit(num_threads: Option<usize>) -> usize {
    match num_threads {
        Some(0) => 1,
        Some(n) => n,
        None => std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_limit_respects_explicit() {
        assert_eq!(resolve_limit(Some(4)), 4);
        assert_eq!(resolve_limit(Some(1)), 1);
        assert_eq!(resolve_limit(Some(0)), 1);
    }

    #[test]
    fn resolve_limit_default_at_least_one() {
        assert!(resolve_limit(None) >= 1);
    }

    #[tokio::test]
    async fn gate_limits_concurrent_holders() {
        let gate = Arc::new(ConcurrencyGate::from_num_threads(Some(1)));
        let p1 = gate.acquire().await;
        let gate2 = Arc::clone(&gate);
        let handle = tokio::spawn(async move {
            let _p2 = gate2.acquire().await;
            true
        });
        tokio::task::yield_now().await;
        assert!(!handle.is_finished());
        drop(p1);
        assert!(handle.await.unwrap());
    }
}
