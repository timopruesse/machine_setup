pub mod clone;
pub mod copy;
pub mod fs_ops;
pub mod run;
pub mod setup;
pub mod symlink;
pub mod tree;

use async_trait::async_trait;

use crate::config::types::CommandEntry;
use crate::error::Result;

use super::context::CommandContext;

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
}

/// Create a command executor from a config entry. Takes ownership so the
/// args struct moves directly into the executor without an intermediate
/// clone inside each match arm.
pub fn create_executor(entry: CommandEntry) -> Box<dyn CommandExecutor> {
    match entry {
        CommandEntry::Copy(args) => Box::new(copy::CopyCommand::new(args)),
        CommandEntry::Symlink(args) => Box::new(symlink::SymlinkCommand::new(args)),
        CommandEntry::Clone(args) => Box::new(clone::CloneCommand::new(args)),
        CommandEntry::Run(args) => Box::new(run::RunCommand::new(args)),
        CommandEntry::MachineSetup(args) => Box::new(setup::SetupCommand::new(args)),
    }
}
