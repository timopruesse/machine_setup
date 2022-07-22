use std::{env, str::FromStr};

use ansi_term::Color::{Green, Red, White, Yellow};

use crate::{
    command::{get_command, CommandConfig, CommandInterface},
    config::{base_config::Command, config_value::ConfigValue, os::Os},
    task_runner::TaskRunnerMode,
    utils::threads::ThreadPool,
};
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

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

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub commands: Vec<Command>,
    pub os: Vec<Os>,
    pub parallel: bool,
}

impl Task {
    pub fn run(&self, mode: &TaskRunnerMode, config: &CommandConfig) -> Result<(), ()> {
        if should_skip_task(self) {
            println!(
                "{}",
                Yellow.bold().paint(format!(
                    "Skipping task \"{}\" due to OS condition ...",
                    self.name
                ))
            );

            return Ok(());
        }

        println!(
            "\nRunning task {} ...\n",
            White.bold().paint(self.name.to_string())
        );

        let commands = &self.commands;

        let num_threads = if self.parallel { commands.len() } else { 1 };

        if self.parallel {
            println!(
                "Executing commands in parallel ({} threads)...",
                White.bold().paint(num_threads.to_string())
            );
        }

        let thread_pool = ThreadPool::new(num_threads);

        for command in commands.clone() {
            let m = mode.clone();
            let c = config.clone();

            let run = move || {
                let resolved_command = get_command(&command.name);
                if resolved_command.is_err() {
                    eprintln!(
                        "{} {} {}",
                        Red.paint("Command"),
                        White.on(Red).paint(format!(" {} ", command.name)),
                        Red.paint("not found")
                    );
                }

                let result = run_command(resolved_command.unwrap(), command.args.clone(), &m, &c);

                if let Err(err_result) = result {
                    eprintln!(
                        "{}: {}",
                        White.bold().paint(command.name.to_string()),
                        Red.paint("ERROR")
                    );
                    err_result
                        .split('\n')
                        .for_each(|err| eprintln!("{} {}", Red.bold().paint("|>"), Red.paint(err)));
                }

                println!(
                    "\n{}: {}",
                    White.bold().paint(command.name.to_string()),
                    Green.bold().paint("OK")
                );
            };

            thread_pool.execute(run);
        }

        Ok(())
    }
}

pub fn get_task_names(tasks: &[Task]) -> Vec<String> {
    let mut task_names = Vec::new();
    for task in tasks {
        task_names.push(task.name.clone());
    }

    task_names
}

pub fn select_task(tasks: &[Task]) -> Option<String> {
    println!("\n{}:", White.bold().paint("Select a task"));

    let task_names = get_task_names(tasks);

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&task_names)
        .default(0)
        .interact_on_opt(&Term::stderr());

    if let Ok(Some(selected_index)) = selection {
        return Some(task_names[selected_index].clone());
    }

    None
}

pub fn should_skip_task(task: &Task) -> bool {
    if task.os.is_empty() {
        return false;
    }

    let current_os = Os::from_str(env::consts::OS).unwrap();

    !task.os.contains(&current_os)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_gets_list_of_tasks() {
        let tasks = vec![
            Task {
                name: "task1".to_string(),
                commands: vec![],
                os: vec![],
                parallel: false,
            },
            Task {
                name: "task2".to_string(),
                commands: vec![],
                os: vec![],
                parallel: false,
            },
        ];

        let task_names = get_task_names(&tasks);

        assert_eq!(task_names, vec!["task1", "task2"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn it_only_runs_task_for_specific_os() {
        let task_linux = Task {
            os: vec![Os::Linux],
            name: String::from("my-linux-task"),
            commands: vec![],
            parallel: false,
        };
        assert!(!should_skip_task(&task_linux));

        let task_win = Task {
            os: vec![Os::Windows],
            name: String::from("my-linux-task"),
            commands: vec![],
            parallel: false,
        };
        assert!(should_skip_task(&task_win));
    }
}
