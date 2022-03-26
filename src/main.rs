pub mod command;
pub mod commands;
pub mod config;
pub mod task_runner;
pub mod utils;

use clap::{Parser, Subcommand};
use ergo_fs::{expand, PathBuf};

use crate::task_runner::TaskRunnerMode;

/// Machine Setup CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Args {
    /// what should be done?
    #[clap(subcommand)]
    mode: Option<TaskRunnerMode>,

    /// path to the config file
    #[clap(short, long, default_value = "./machine_setup.yaml")]
    #[clap(global = true)]
    file: String,
}

fn main() {
    let args = Args::parse();

    let config_path = expand(&args.file);
    if config_path.is_err() {
        eprintln!("{}", config_path.err().unwrap());
        std::process::exit(1);
    }

    let mode = &args.mode.unwrap_or(TaskRunnerMode::Install);

    let run = task_runner::run(&config_path.unwrap(), TaskRunnerMode::Install);

    if run.is_err() {
        eprintln!("{}", run.unwrap_err());
        std::process::exit(1);
    }

    println!("... DONE ...");
}
