pub mod catalog;
pub mod clone;
pub mod copy;
pub mod fs_ops;
pub mod ignore;
pub mod progress_log;
pub mod run;
pub mod setup;
pub mod symlink;
pub mod tree;
pub mod tree_op;

use async_trait::async_trait;

use crate::error::Result;

use super::context::CommandContext;

pub use catalog::{create_executor, exclusive_lane};
pub use clone::CloneCommand;
pub use copy::CopyCommand;
pub use run::RunCommand;
pub use setup::SetupCommand;
pub use symlink::SymlinkCommand;

/// Closed set of Command executors — one variant per Command entry kind.
///
/// The Command kind catalog is closed (ADR-0006); an enum gives static
/// dispatch at the Runner seam instead of `Box<dyn CommandExecutor>`.
pub enum Executor {
    Copy(CopyCommand),
    Symlink(SymlinkCommand),
    Clone(CloneCommand),
    Run(RunCommand),
    MachineSetup(SetupCommand),
}

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

#[async_trait]
impl CommandExecutor for Executor {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        match self {
            Self::Copy(c) => c.execute(ctx).await,
            Self::Symlink(c) => c.execute(ctx).await,
            Self::Clone(c) => c.execute(ctx).await,
            Self::Run(c) => c.execute(ctx).await,
            Self::MachineSetup(c) => c.execute(ctx).await,
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Copy(c) => c.description(),
            Self::Symlink(c) => c.description(),
            Self::Clone(c) => c.description(),
            Self::Run(c) => c.description(),
            Self::MachineSetup(c) => c.description(),
        }
    }

    fn occupies_concurrency_slot(&self) -> bool {
        match self {
            Self::MachineSetup(c) => c.occupies_concurrency_slot(),
            _ => true,
        }
    }
}
