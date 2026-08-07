pub mod catalog;
pub mod clone;
pub mod copy;
pub mod fs_ops;
pub mod progress_log;
pub mod run;
pub mod setup;
pub mod symlink;
pub mod tree;
pub mod tree_op;

use async_trait::async_trait;

use crate::error::Result;

use super::context::CommandContext;

pub use catalog::create_executor;

/// Trait for executable commands.
///
/// A single `execute` entry point reads the execution mode from `ctx.mode`,
/// so mode dispatch happens exactly once (inside the executor) rather than
/// once here and again at the call site.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(&self, ctx: &CommandContext) -> Result<()>;

    /// Short description for display.
    fn description(&self) -> String;

    /// Whether this Command entry holds a ConcurrencyGate permit while
    /// `execute` runs. `machine_setup` returns false so a nested Runner can
    /// acquire on the shared gate without deadlocking (ADR-0003).
    fn occupies_concurrency_slot(&self) -> bool {
        true
    }
}
