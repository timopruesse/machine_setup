extern crate tracing;

use clap::Parser;
use terminal::{cli::Args, command::execute_command};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub mod command;
pub mod commands;
pub mod config;
pub mod task;
pub mod task_runner;
pub mod terminal;
pub mod utils;

fn main() {
    let args = Args::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .pretty()
        .with_level(args.debug)
        .with_line_number(args.debug)
        .with_file(args.debug)
        .with_target(args.debug)
        .without_time()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Could not set default subscriber...");

    execute_command(args)
}
