//! Concurrency gate — global cap on in-flight Task / Command executor work.
//!
//! See ADR-0003. Does not order Tasks by dependency (that is the Task graph).
//! Also owns the shared Rayon pool used for in-tree file apply (ADR-0004);
//! the pool is created lazily on first [`ConcurrencyGate::pool`] call.

use std::fmt;
use std::sync::{Arc, OnceLock};

use rayon::ThreadPool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Package-manager family for an **Exclusive lane** (ADR-0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExclusiveLane {
    Apt,
    Brew,
    Dnf,
    Pacman,
    Apk,
    Winget,
    Choco,
}

impl ExclusiveLane {
    const COUNT: usize = 7;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Brew => "brew",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Apk => "apk",
            Self::Winget => "winget",
            Self::Choco => "choco",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for ExclusiveLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn lane_semaphores() -> [Arc<Semaphore>; ExclusiveLane::COUNT] {
    std::array::from_fn(|_| Arc::new(Semaphore::new(1)))
}

/// Shared limit on concurrent Task and Command executor work.
#[derive(Clone)]
pub struct ConcurrencyGate {
    sem: Arc<Semaphore>,
    /// Configured permit count (for tests / diagnostics).
    pub limit: usize,
    /// Shared FS apply pool — sized by `limit`, created on first use.
    pool: Arc<OnceLock<ThreadPool>>,
    /// One permit per Exclusive lane family (intra-run serialization).
    lanes: [Arc<Semaphore>; ExclusiveLane::COUNT],
}

impl ConcurrencyGate {
    /// Build a gate from `AppConfig.num_threads` (None → CPUs − 1, at least 1).
    ///
    /// Does not start Rayon workers; call [`Self::pool`] when tree apply needs them.
    pub fn from_num_threads(num_threads: Option<usize>) -> Self {
        let limit = resolve_limit(num_threads);
        Self {
            sem: Arc::new(Semaphore::new(limit)),
            limit,
            pool: Arc::new(OnceLock::new()),
            lanes: lane_semaphores(),
        }
    }

    /// Shared Rayon pool for in-tree DirectFs file apply.
    ///
    /// Initializes the pool on first call (thread-safe), sized by `limit`.
    pub fn pool(&self) -> &ThreadPool {
        self.pool.get_or_init(|| build_fs_pool(self.limit))
    }

    /// Acquire one permit; held for the lifetime of the returned guard.
    pub async fn acquire(self: &Arc<Self>) -> OwnedSemaphorePermit {
        #[expect(
            clippy::expect_used,
            reason = "ConcurrencyGate semaphore is never closed"
        )]
        self.sem
            .clone()
            .acquire_owned()
            .await
            .expect("ConcurrencyGate semaphore is never closed")
    }

    /// Try to take an Exclusive lane without waiting.
    pub fn try_acquire_lane(&self, lane: ExclusiveLane) -> Option<OwnedSemaphorePermit> {
        self.lanes[lane.index()].clone().try_acquire_owned().ok()
    }

    /// Wait until the Exclusive lane is free.
    pub async fn acquire_lane(&self, lane: ExclusiveLane) -> OwnedSemaphorePermit {
        #[expect(
            clippy::expect_used,
            reason = "Exclusive lane semaphore is never closed"
        )]
        self.lanes[lane.index()]
            .clone()
            .acquire_owned()
            .await
            .expect("Exclusive lane semaphore is never closed")
    }
}

fn build_fs_pool(limit: usize) -> ThreadPool {
    #[expect(
        clippy::expect_used,
        reason = "Rayon pool build only fails on invalid config we control"
    )]
    rayon::ThreadPoolBuilder::new()
        .num_threads(limit)
        .thread_name(|i| format!("machine-setup-fs-{i}"))
        .build()
        .expect("ConcurrencyGate Rayon pool")
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

    #[test]
    fn fs_pool_lazy_until_first_pool_call() {
        let gate = ConcurrencyGate::from_num_threads(Some(2));
        assert!(gate.pool.get().is_none());
        let _ = gate.pool();
        assert!(gate.pool.get().is_some());
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

    #[tokio::test]
    async fn second_apt_lane_blocks_until_first_drops() {
        let gate = Arc::new(ConcurrencyGate::from_num_threads(Some(2)));
        let held = gate.try_acquire_lane(ExclusiveLane::Apt).unwrap();
        let gate2 = Arc::clone(&gate);
        let handle = tokio::spawn(async move { gate2.acquire_lane(ExclusiveLane::Apt).await });
        tokio::task::yield_now().await;
        assert!(!handle.is_finished());
        drop(held);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn brew_lane_does_not_block_apt() {
        let gate = ConcurrencyGate::from_num_threads(Some(2));
        let _apt = gate.try_acquire_lane(ExclusiveLane::Apt).unwrap();
        assert!(gate.try_acquire_lane(ExclusiveLane::Brew).is_some());
    }

    #[tokio::test]
    async fn lane_waiter_does_not_consume_work_permit() {
        let gate = Arc::new(ConcurrencyGate::from_num_threads(Some(1)));
        let _lane = gate.try_acquire_lane(ExclusiveLane::Apt).unwrap();
        let gate2 = Arc::clone(&gate);
        let waiter = tokio::spawn(async move { gate2.acquire_lane(ExclusiveLane::Apt).await });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        let _permit = gate.acquire().await;
        waiter.abort();
    }
}
