use core::fmt;
use yaml_rust::Yaml;

use crate::{
    command::{get_command, CommandInterface},
    config::base_config::{Task, TaskList},
    utils::shell::Shell,
};

#[derive(Debug)]
pub enum TaskRunnerMode {
    Install,
    Update,
    Uninstall,
}

impl fmt::Display for TaskRunnerMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mode = match self {
            TaskRunnerMode::Install => "install",
            TaskRunnerMode::Update => "update",
            TaskRunnerMode::Uninstall => "uninstall",
        };

        write!(f, "{}", mode)
    }
}

fn run_command(
    command: Box<dyn CommandInterface>,
    args: Yaml,
    mode: &TaskRunnerMode,
    temp_dir: &str,
    default_shell: &Shell,
) -> Result<(), String> {
    match mode {
        TaskRunnerMode::Install => command.install(args, temp_dir, default_shell),
        TaskRunnerMode::Update => command.update(args, temp_dir, default_shell),
        TaskRunnerMode::Uninstall => command.uninstall(args, temp_dir, default_shell),
    }
}

fn run_task(task: &Task, mode: &TaskRunnerMode, temp_dir: &str, default_shell: &Shell) {
    println!("\nRunning task \"{}\" ...", task.name);

    let commands = &task.commands;
    for command in commands {
        let resolved_command = get_command(&command.name);
        if resolved_command.is_err() {
            eprintln!("Command \"{}\" not found", command.name);
            continue;
        }

        let result = run_command(
            resolved_command.unwrap(),
            command.args.clone(),
            mode,
            temp_dir,
            default_shell,
        );

        if let Err(err_result) = result {
            eprintln!("{}: ERROR", command.name);
            err_result.split('\n').for_each(|err| eprintln!("{}", err));
        } else {
            println!("{}: OK", command.name);
        }
    }
}

pub fn run(
    task_list: TaskList,
    mode: TaskRunnerMode,
    task_name: Option<String>,
) -> Result<(), String> {
    match mode {
        TaskRunnerMode::Install => println!("\nInstalling..."),
        TaskRunnerMode::Update => println!("\nUpdating..."),
        TaskRunnerMode::Uninstall => println!("\nUninstalling..."),
    }

    if let Some(task_name) = task_name {
        let task = task_list.tasks.iter().find(|t| t.name == task_name);
        if task.is_none() {
            return Err(format!("Task \"{}\" not found", task_name));
        }

        run_task(
            task.unwrap(),
            &mode,
            task_list.temp_dir.as_str(),
            &task_list.default_shell,
        );

        return Ok(());
    }

    for task in task_list.tasks {
        run_task(
            &task,
            &mode,
            task_list.temp_dir.as_str(),
            &task_list.default_shell,
        );
    }

    Ok(())
}
