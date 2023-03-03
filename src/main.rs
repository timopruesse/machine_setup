extern crate tracing;

use clap::Parser;
use once_cell::sync::OnceCell;
use terminal::{cli::Args, command::execute_command};
use tracing::metadata::LevelFilter;
use tracing::{Level, error};
use tracing_subscriber::prelude::*;

pub mod command;
pub mod commands;
pub mod config;
pub mod task;
pub mod task_runner;
pub mod terminal;
pub mod utils;

static LOG_LEVEL: OnceCell<Level> = OnceCell::new();
static DEBUG_MODE: OnceCell<bool> = OnceCell::new();

fn main() {
    let args = Args::parse();

    LOG_LEVEL.set(args.level).unwrap_or_default();
    DEBUG_MODE.set(args.debug).unwrap_or_default();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_level(args.debug)
        .with_line_number(args.debug)
        .with_file(args.debug)
        .with_target(args.debug)
        .with_thread_ids(args.debug)
        .without_time();

    let subscriber = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(LevelFilter::from_level(args.level))
        .try_init();

    if let Err(sub_err) = subscriber {
        error!("{sub_err:?}");
    }


    execute_command(args)
}
