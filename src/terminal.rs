use ansi_term::Color::{Red, White};
use clap::Parser;
use clap::Subcommand;
use ergo_fs::expand;
use std::str::FromStr;

use crate::config::base_config::get_config;
use crate::config::base_config::Task;
use crate::task::get_task_names;
use crate::task::select_task;
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
    command: SubCommand,

    /// path to the config file
    #[clap(short, long, default_value = "./machine_setup.yaml")]
    #[clap(global = true)]
    config: String,

    /// run a single task
    #[clap(short, long)]
    #[clap(global = true)]
    task: Option<String>,

    /// Select a task to run
    #[clap(short, long)]
    #[clap(global = true)]
    select: bool,
}

fn get_task_runner_mode(subcommand: SubCommand) -> TaskRunnerMode {
    match subcommand {
        SubCommand::Install => TaskRunnerMode::Install,
        SubCommand::Update => TaskRunnerMode::Update,
        SubCommand::Uninstall => TaskRunnerMode::Uninstall,
        _ => panic!("Invalid task runner mode"),
    }
}

fn get_task_from_args(args: &Args, tasks: &[Task]) -> Result<Option<String>, String> {
    if let Some(task_name) = &args.task {
        return Ok(Some(task_name.to_string()));
    }

    if !args.select {
        return Ok(None);
    }

    let task_name = select_task(tasks);
    if task_name.is_none() {
        return Err(format!("{}", Red.paint("No task selected")));
    }

    return Ok(task_name);
}

pub fn execute_command() {
    let args = Args::parse();

    let config_path = expand(&args.config);
    if let Err(err_config_path) = config_path {
        eprintln!("{}", Red.paint(err_config_path.to_string()));
        return;
    }

    let config = get_config(&config_path.unwrap().to_string());

    if let Err(err_config) = config {
        eprintln!("{}", Red.paint(err_config));
        return;
    }

    let task_list = config.unwrap();

    match args.command {
        SubCommand::Install | SubCommand::Uninstall | SubCommand::Update => {
            let task_name = get_task_from_args(&args, &task_list.tasks);

            if let Err(err_task_name) = task_name {
                eprintln!("{}", Red.paint(err_task_name));
                return;
            }

            let mode = get_task_runner_mode(args.command);
            let run = task_runner::run(task_list, mode, task_name.unwrap());

            if run.is_err() {
                eprintln!("{}", Red.paint(run.unwrap_err()));
            }
        }
        SubCommand::List => {
            println!("\nTasks:");
            println!(
                "{}",
                get_task_names(&task_list.tasks)
                    .into_iter()
                    .map(|t| format!("\t> {}", White.bold().paint(t)))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_gets_task_from_args() {
        let args = Args {
            command: SubCommand::Install,
            config: "./machine_setup.yaml".to_string(),
            task: Some("test".to_string()),
            select: false,
        };

        let tasks = vec![Task {
            name: "test".to_string(),
            commands: vec![],
        }];

        let task_name = get_task_from_args(&args, &tasks);

        assert_eq!(task_name.unwrap(), Some("test".to_string()));
    }

    #[test]
    fn it_uses_provided_task_name_instead_of_select() {
        let args = Args {
            command: SubCommand::Install,
            config: "./machine_setup.yaml".to_string(),
            task: Some("test".to_string()),
            select: true,
        };

        let tasks = vec![Task {
            name: "test".to_string(),
            commands: vec![],
        }];

        let task_name = get_task_from_args(&args, &tasks);

        assert_eq!(task_name.unwrap(), Some("test".to_string()));
    }

    #[test]
    fn it_gets_task_runner_mode() {
        let mode = get_task_runner_mode(SubCommand::Install);

        assert_eq!(mode.to_string(), TaskRunnerMode::Install.to_string());
    }
}
