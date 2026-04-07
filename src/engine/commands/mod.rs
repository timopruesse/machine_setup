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

/// Create a command executor from a config entry.
pub fn create_executor(entry: &CommandEntry) -> Box<dyn CommandExecutor> {
    match entry {
        CommandEntry::Copy(args) => Box::new(copy::CopyCommand::new(args.clone())),
        CommandEntry::Symlink(args) => Box::new(symlink::SymlinkCommand::new(args.clone())),
        CommandEntry::Clone(args) => Box::new(clone::CloneCommand::new(args.clone())),
        CommandEntry::Run(args) => Box::new(run::RunCommand::new(args.clone())),
        CommandEntry::MachineSetup(args) => Box::new(setup::SetupCommand::new(args.clone())),
    }
}
