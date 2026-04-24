pub mod clone;
pub mod copy;
pub mod run;
pub mod setup;
pub mod symlink;

use async_trait::async_trait;

use crate::config::types::CommandEntry;
use crate::error::Result;

use super::context::CommandContext;

/// Trait for executable commands.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn install(&self, ctx: &CommandContext) -> Result<()>;
    async fn update(&self, ctx: &CommandContext) -> Result<()>;
    async fn uninstall(&self, ctx: &CommandContext) -> Result<()>;

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
