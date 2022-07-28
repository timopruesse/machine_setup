use std::fs::canonicalize;

use ansi_term::Color::{Red, White};
use ergo_fs::expand;
use ergo_fs::Path;
use ergo_fs::PathDir;
use tracing::error;

use crate::config::base_config::get_config;
use crate::task::get_task_names;
use crate::task::select_task;
use crate::task::Task;
use crate::task_runner;
use crate::task_runner::TaskRunnerMode;

use super::cli::Args;
use super::cli::SubCommand;

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

    Ok(task_name)
}

pub fn execute_command(args: Args) {
    let config_path = expand(&args.config);
    if let Err(err_config_path) = config_path {
        error!("{}", Red.paint(err_config_path.to_string()));
        return;
    }
    let config_path = config_path.unwrap();

    let config = get_config(&config_path);
    if let Err(err_config) = config {
        error!("{}", Red.paint(err_config));
        return;
    }

    let task_list = config.unwrap();

    match args.command {
        SubCommand::Install | SubCommand::Uninstall | SubCommand::Update => {
            let task_name = get_task_from_args(&args, &task_list.tasks);

            if let Err(err_task_name) = task_name {
                error!("{}", Red.paint(err_task_name));
                return;
            }

            let mode = get_task_runner_mode(args.command);
            let run = task_runner::run(
                task_list,
                mode,
                task_name.unwrap(),
                PathDir::new(
                    canonicalize(Path::new(&config_path.to_string()).parent().unwrap())
                        .unwrap()
                        .as_path(),
                )
                .unwrap(),
            );

            if run.is_err() {
                error!("{}", Red.paint(run.unwrap_err()));
            }
        }
        SubCommand::List => {
            println!(
                "\n\tTasks\n\t--------------------------------\n{}\n\t--------------------------------",
                get_task_names(&task_list.tasks)
                    .into_iter()
                    .map(|t| format!("\t|> {}", White.bold().paint(t)))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }
    }
}

#[cfg(test)]
mod test {
    use tracing::Level;

    use super::*;

    #[test]
    fn it_gets_task_from_args() {
        let args = Args {
            command: SubCommand::Install,
            config: "./machine_setup.yaml".to_string(),
            task: Some("test".to_string()),
            select: false,
            level: Level::INFO,
            debug: true,
        };

        let tasks = vec![Task {
            name: "test".to_string(),
            commands: vec![],
            os: vec![],
            parallel: false,
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
            level: Level::INFO,
            debug: true,
        };

        let tasks = vec![Task {
            name: "test".to_string(),
            commands: vec![],
            os: vec![],
            parallel: false,
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
