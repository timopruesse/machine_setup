use std::{env, str::FromStr};

use ansi_term::Color::White;

use crate::config::{base_config::Task, os::Os};
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

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
    use crate::config::base_config::Task;

    use super::*;

    #[test]
    fn it_gets_list_of_tasks() {
        let tasks = vec![
            Task {
                name: "task1".to_string(),
                commands: vec![],
                os: vec![],
            },
            Task {
                name: "task2".to_string(),
                commands: vec![],
                os: vec![],
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
        };
        assert!(!should_skip_task(&task_linux));

        let task_win = Task {
            os: vec![Os::Windows],
            name: String::from("my-linux-task"),
            commands: vec![],
        };
        assert!(should_skip_task(&task_win));
    }
}
