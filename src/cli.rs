use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "machine_setup",
    version,
    about = "Automate machine configuration and setup tasks"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config file (YAML/JSON auto-detected)
    #[arg(short, long, global = true, default_value = "./machine_setup")]
    pub config: PathBuf,

    /// Run only a specific task by name
    #[arg(short, long, global = true)]
    pub task: Option<String>,

    /// Interactively select tasks to run
    #[arg(short, long, global = true)]
    pub select: bool,

    /// Force execution (bypass history checks)
    #[arg(short, long, global = true)]
    pub force: bool,

    /// Disable TUI (plain log output)
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Enable debug output
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Log level
    #[arg(short, long, global = true, default_value = "warn")]
    pub level: String,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Install all or selected tasks
    Install,
    /// Update all or selected tasks
    Update,
    /// Uninstall all or selected tasks
    Uninstall,
    /// List all defined tasks
    List,
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Install => write!(f, "install"),
            Command::Update => write!(f, "update"),
            Command::Uninstall => write!(f, "uninstall"),
            Command::List => write!(f, "list"),
        }
    }
}
