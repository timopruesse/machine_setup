use clap::Subcommand;
use core::fmt;
use std::str::FromStr;
use yaml_rust::Yaml;

use crate::{
    command::{get_command, CommandInterface},
    config::{
        base_config::{BaseConfig, Task},
        yaml_config::YamlConfig,
    },
};

#[derive(Subcommand, Debug)]
pub enum TaskRunnerMode {
    /// Install all of the defined tasks
    Install,

    /// Update all of the defined tasks
    Update,

    /// Uninstall all of the defined tasks
    Uninstall,
}

impl fmt::Display for TaskRunnerMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl FromStr for TaskRunnerMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "install" => Ok(TaskRunnerMode::Install),
            "update" => Ok(TaskRunnerMode::Update),
            "uninstall" => Ok(TaskRunnerMode::Uninstall),
            _ => Err(format!("Invalid mode: {}", s)),
        }
    }
}

fn get_config_handler(config_path: &str) -> Result<Box<dyn BaseConfig>, String> {
    let file_ending = config_path.split('.').last().unwrap();

    return match file_ending {
        file_ending if file_ending == "yml" || file_ending == "yaml" => Ok(Box::new(YamlConfig {})),
        _ => Err(format!("Unsupported config file format: .{}", file_ending)),
    };
}

fn run_command(
    command: Box<dyn CommandInterface>,
    args: Yaml,
    mode: &TaskRunnerMode,
) -> Result<(), String> {
    return match mode {
        TaskRunnerMode::Install => command.install(args),
        TaskRunnerMode::Update => command.update(args),
        TaskRunnerMode::Uninstall => command.uninstall(args),
    };
}

fn run_task(task: &Task, mode: &TaskRunnerMode) {
    println!("Running task \"{}\" ...", task.name);

    let commands = &task.commands;
    for command in commands {
        let resolved_command = get_command(&command.name);
        if resolved_command.is_err() {
            eprintln!("Command \"{}\" not found", command.name);
            continue;
        }

        let result = run_command(resolved_command.unwrap(), command.args.clone(), &mode);

        if result.is_err() {
            eprintln!("{}: ERROR", command.name);
            eprintln!("{:?}", result.unwrap_err());
        } else {
            println!("{}: OK", command.name);
        }
    }
}

pub fn run(
    config_path: &str,
    mode: TaskRunnerMode,
    task_name: Option<String>,
) -> Result<(), String> {
    let config = get_config_handler(config_path);
    if config.is_err() {
        return Err(config.err().unwrap());
    }
    let config = config.unwrap();

    let result = config.read(config_path);

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    match mode {
        TaskRunnerMode::Install => println!("Installing..."),
        TaskRunnerMode::Update => println!("Updating..."),
        TaskRunnerMode::Uninstall => println!("Uninstalling..."),
    }

    let task_list = result.unwrap().tasks;

    if task_name.is_some() {
        let task_name = task_name.unwrap();
        let task = task_list.iter().find(|t| t.name == task_name);
        if task.is_none() {
            return Err(format!("Task \"{}\" not found", task_name));
        }

        run_task(task.unwrap(), &mode);

        return Ok(());
    }

    for task in task_list {
        run_task(&task, &mode);
    }

    return Ok(());
}
