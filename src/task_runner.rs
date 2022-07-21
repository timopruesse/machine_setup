use ansi_term::Color::{Green, Red, White, Yellow};
use core::fmt;
use ergo_fs::PathDir;

use crate::{
    command::{get_command, CommandConfig, CommandInterface},
    config::{
        base_config::{Task, TaskList},
        config_value::ConfigValue,
    },
    task::should_skip_task,
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
    args: ConfigValue,
    mode: &TaskRunnerMode,
    config: &CommandConfig,
) -> Result<(), String> {
    match mode {
        TaskRunnerMode::Install => command.install(args, config),
        TaskRunnerMode::Update => command.update(args, config),
        TaskRunnerMode::Uninstall => command.uninstall(args, config),
    }
}

fn run_task(task: &Task, mode: &TaskRunnerMode, config: &CommandConfig) -> Result<(), ()> {
    if should_skip_task(task) {
        println!(
            "{}",
            Yellow.bold().paint(format!(
                "Skipping task \"{}\" due to OS condition ...",
                task.name
            ))
        );

        return Ok(());
    }

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
            config,
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
    config_dir: PathDir,
) -> Result<(), String> {
    match mode {
        TaskRunnerMode::Install => println!("{}", White.bold().paint("\nInstalling...")),
        TaskRunnerMode::Update => println!("{}", White.bold().paint("\nUpdating...")),
        TaskRunnerMode::Uninstall => println!("{}", White.bold().paint("\nUninstalling...")),
    }

    let command_config = CommandConfig {
        config_dir,
        temp_dir: task_list.temp_dir.to_string(),
        default_shell: task_list.default_shell,
    };

    if let Some(task_name) = task_name {
        let task = task_list.tasks.iter().find(|t| t.name == task_name);
        if task.is_none() {
            return Err(format!(
                "\nTask {} {}",
                White.on(Red).paint(format!(" {} ", task_name)),
                Red.paint("not found")
            ));
        }

        let task_result = run_task(task.unwrap(), &mode, &command_config);
        if task_result.is_err() {
            return Err(format!(
                "\nTask {} {}",
                White.on(Red).paint(format!(" {} ", task_name)),
                Red.paint("failed")
            ));
        }

        return Ok(());
    }

    println!("NUM: {}", task_list.num_threads);

    let mut errored_tasks = vec![];
    for task in task_list.tasks {
        println!("parallel? -> {:?}", task.parallel);

        let task_result = run_task(&task, &mode, &command_config);

        if task_result.is_err() {
            errored_tasks.push(task.name.to_string());
        }
    }

    let num_errored = errored_tasks.len();
    if num_errored > 0 {
        return Err(format!(
            "\n{} {} {}\n{}",
            Red.paint("Errors occurred in"),
            Red.bold().underline().paint(num_errored.to_string()),
            Red.paint("tasks:"),
            errored_tasks
                .into_iter()
                .map(|e| format!("> {}", e))
                .collect::<Vec<String>>()
                .join("\n")
        ));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, env::temp_dir};

    use crate::{config::base_config::Command, utils::shell::Shell};

    use super::*;

    fn get_temp_path_dir() -> PathDir {
        PathDir::new(temp_dir()).unwrap()
    }

    #[test]
    fn it_runs_single_task_when_argument_is_passed() {
        let task_list = TaskList {
            tasks: vec![
                Task {
                    name: "task_one".to_string(),
                    commands: vec![Command {
                        name: "_TEST_".to_string(),
                        args: ConfigValue::Array(vec![]),
                    }],
                    os: vec![],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![],
                    os: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            Some("task_one".to_string()),
            get_temp_path_dir(),
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

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            Some("test".to_string()),
            get_temp_path_dir(),
        );

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
                    os: vec![],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![],
                    os: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            None,
            get_temp_path_dir(),
        );

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
                        args: ConfigValue::Array(vec![]),
                    }],
                    os: vec![],
                },
                Task {
                    name: "task_two".to_string(),
                    commands: vec![Command {
                        name: "_TEST_".to_string(),
                        args: ConfigValue::Array(vec![]),
                    }],
                    os: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            None,
            get_temp_path_dir(),
        );

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Errors occurred in"));
        assert!(error_message.contains("task_one"));
        assert!(error_message.contains("task_two"));
    }

    #[test]
    fn it_runs_commands() {
        let mut run_commands = HashMap::new();
        run_commands.insert(
            String::from("commands"),
            ConfigValue::String(String::from("echo \"test\"")),
        );

        let command = Command {
            name: String::from("run"),
            args: ConfigValue::Hash(run_commands),
        };

        let task_list = TaskList {
            tasks: vec![Task {
                name: "task_one".to_string(),
                commands: vec![command],
                os: vec![],
            }],
            temp_dir: temp_dir().to_str().unwrap().to_string(),
            default_shell: Shell::Bash,
        };

        let result = run(
            task_list,
            TaskRunnerMode::Install,
            None,
            get_temp_path_dir(),
        );

        assert!(result.is_ok());
    }
}
