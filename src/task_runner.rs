use ansi_term::Color::{Green, Red, White};
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

fn run_task(
    task: &Task,
    mode: &TaskRunnerMode,
    temp_dir: &str,
    default_shell: &Shell,
) -> Result<(), ()> {
    println!(
        "\nRunning task {} ...\n",
        White.bold().paint(task.name.to_string())
    );

    let commands = &task.commands;
    for command in commands {
        let resolved_command = get_command(&command.name);
        if resolved_command.is_err() {
            eprintln!(
                "{} {} {}",
                Red.paint("Command"),
                White.on(Red).paint(format!(" {} ", command.name)),
                Red.paint("not found")
            );
            return Err(());
        }

        let result = run_command(
            resolved_command.unwrap(),
            command.args.clone(),
            mode,
            temp_dir,
            default_shell,
        );

        if let Err(err_result) = result {
            eprintln!(
                "{}: {}",
                White.bold().paint(command.name.to_string()),
                Red.paint("ERROR")
            );
            err_result
                .split('\n')
                .for_each(|err| eprintln!("{} {}", Red.bold().paint("|>"), Red.paint(err)));

            return Err(());
        }

        println!(
            "\n{}: {}",
            White.bold().paint(command.name.to_string()),
            Green.bold().paint("OK")
        );
    }

    Ok(())
}

pub fn run(
    task_list: TaskList,
    mode: TaskRunnerMode,
    task_name: Option<String>,
) -> Result<(), String> {
    match mode {
        TaskRunnerMode::Install => println!("{}", White.bold().paint("\nInstalling...")),
        TaskRunnerMode::Update => println!("{}", White.bold().paint("\nUpdating...")),
        TaskRunnerMode::Uninstall => println!("{}", White.bold().paint("\nUninstalling...")),
    }

    if let Some(task_name) = task_name {
        let task = task_list.tasks.iter().find(|t| t.name == task_name);
        if task.is_none() {
            return Err(format!(
                "\nTask {} {}",
                White.on(Red).paint(format!(" {} ", task_name)),
                Red.paint("not found")
            ));
        }

        let task_result = run_task(
            task.unwrap(),
            &mode,
            task_list.temp_dir.as_str(),
            &task_list.default_shell,
        );

        if let Err(_) = task_result {
            return Err(format!(
                "\nTask {} {}",
                White.on(Red).paint(format!(" {} ", task_name)),
                Red.paint("failed")
            ));
        }

        return Ok(());
    }

    let mut errored_tasks = vec![];
    for task in task_list.tasks {
        let task_result = run_task(
            &task,
            &mode,
            task_list.temp_dir.as_str(),
            &task_list.default_shell,
        );

        if let Err(_) = task_result {
            errored_tasks.push(task.name.to_string());
        }
    }

    if errored_tasks.len() > 0 {
        return Err(format!(
            "\n{} {}",
            Red.paint("Errors occurred while running tasks:"),
            errored_tasks.join(", ")
        ));
    }

    Ok(())
}

// --- tests ---

mod tests {
    use crate::config::base_config::Command;

    use super::*;

    #[test]
    fn it_runs_single_task_when_argument_is_passed() {
        let task_list = TaskList {
            tasks: vec![
                Task {
                    name: "task_one".to_string(),
                    commands: vec![Command {
                        name: "_TEST_".to_string(),
                        args: Yaml::Array(vec![]),
                    }],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            Some("task_one".to_string()),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("task_one"));
    }

    #[test]
    fn it_fails_when_the_task_doesnt_exist() {
        let task_list = TaskList {
            tasks: vec![],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(task_list, TaskRunnerMode::Install, Some("test".to_string()));

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("test"));
        assert!(error_message.contains("not found"));
    }

    #[test]
    fn it_runs_all_tasks_when_no_argument_is_passed() {
        let task_list = TaskList {
            tasks: vec![
                Task {
                    name: "task_one".to_string(),
                    commands: vec![],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(task_list, TaskRunnerMode::Install, None);

        assert!(result.is_ok());
    }

    #[test]
    fn it_prints_failing_tasks() {
        let task_list = TaskList {
            tasks: vec![
                Task {
                    name: "task_one".to_string(),
                    commands: vec![Command {
                        name: "_TEST_".to_string(),
                        args: Yaml::Array(vec![]),
                    }],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![Command {
                        name: "_TEST_".to_string(),
                        args: Yaml::Array(vec![]),
                    }],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(task_list, TaskRunnerMode::Install, None);

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Errors occurred while running tasks"));
        assert!(error_message.contains("task_one"));
        assert!(error_message.contains("task_two"));
    }
}
