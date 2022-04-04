use clap::Parser;
use clap::Subcommand;
use ergo_fs::expand;
use std::str::FromStr;

use crate::config::base_config::get_config;
use crate::task::get_task_names;
use crate::task_runner;
use crate::task_runner::TaskRunnerMode;

#[derive(Subcommand, Debug)]
enum SubCommand {
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
struct Args {
    /// what should be done?
    #[clap(subcommand)]
    command: Option<SubCommand>,

    /// path to the config file
    #[clap(short, long, default_value = "./machine_setup.yaml")]
    #[clap(global = true)]
    config: String,

    /// run a single task
    #[clap(short, long)]
    #[clap(global = true)]
    task: Option<String>,
}

fn get_task_runner_mode(subcommand: SubCommand) -> TaskRunnerMode {
    match subcommand {
        SubCommand::Install => TaskRunnerMode::Install,
        SubCommand::Update => TaskRunnerMode::Update,
        SubCommand::Uninstall => TaskRunnerMode::Uninstall,
        _ => panic!("Invalid task runner mode"),
    }
}

pub fn execute_command() {
    let args = Args::parse();

    let config_path = expand(&args.config);
    if let Err(err_config_path) = config_path {
        eprintln!("{}", err_config_path);
        std::process::exit(1);
    }

    let config = get_config(&config_path.unwrap().to_string());

    if let Err(err_config) = config {
        eprintln!("{}", err_config);
        std::process::exit(1);
    }

    let task_list = config.unwrap();
    let subcommand = args.command.unwrap_or(SubCommand::Install);

    match subcommand {
        SubCommand::Install | SubCommand::Uninstall | SubCommand::Update => {
            let mode = get_task_runner_mode(subcommand);
            let run = task_runner::run(task_list, mode, args.task);

            if run.is_err() {
                eprintln!("{}", run.unwrap_err());
                std::process::exit(1);
            }
        }
        SubCommand::List => {
            println!("\nTasks:");
            println!("{}", get_task_names(task_list).join("\n"));
        }
    }
}
