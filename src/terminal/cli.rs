use clap::{Parser, Subcommand};
use std::str::FromStr;

#[derive(Subcommand, Debug)]
pub enum SubCommand {
    /// Install all of the defined tasks
    Install,

    /// Update all of the defined tasks
    Update,

    /// Uninstall all of the defined tasks
    Uninstall,

    /// List defined tasks
    List,
}

impl FromStr for SubCommand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "install" => Ok(SubCommand::Install),
            "update" => Ok(SubCommand::Update),
            "uninstall" => Ok(SubCommand::Uninstall),
            "list" => Ok(SubCommand::List),
            _ => Err(format!("Invalid mode: {}", s)),
        }
    }
}

/// Machine Setup CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub struct Args {
    /// what should be done?
    #[clap(subcommand)]
    pub command: SubCommand,

    /// path to the config file
    #[clap(short, long, default_value = "./machine_setup")]
    #[clap(global = true)]
    pub config: String,

    /// run a single task
    #[clap(short, long)]
    #[clap(global = true)]
    pub task: Option<String>,

    /// Select a task to run
    #[clap(short, long)]
    #[clap(global = true)]
    pub select: bool,
}
