use crate::config::base_config::Task;
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

pub fn get_task_names(tasks: &[Task]) -> Vec<String> {
    let mut task_names = Vec::new();
    for task in tasks {
        task_names.push(task.name.clone());
    }

    task_names
}

pub fn select_task(tasks: &[Task]) -> Option<String> {
    println!("\nSelect a task:");

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
            },
            Task {
                name: "task2".to_string(),
                commands: vec![],
            },
        ];

        let task_names = get_task_names(&tasks);

        assert_eq!(task_names, vec!["task1", "task2"]);
    }
}
