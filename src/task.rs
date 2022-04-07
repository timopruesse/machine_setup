use crate::config::base_config::TaskList;

pub fn get_task_names(task_list: TaskList) -> Vec<String> {
    let mut task_names = Vec::new();
    for task in task_list.tasks {
        task_names.push(task.name.clone());
    }

    task_names
}

#[cfg(test)]
mod test {
    use crate::{config::base_config::Task, utils::shell::Shell};

    use super::*;

    #[test]
    fn it_gets_list_of_tasks() {
        let task_list = TaskList {
            tasks: vec![
                Task {
                    name: "task1".to_string(),
                    commands: vec![],
                },
                Task {
                    name: "task2".to_string(),
                    commands: vec![],
                },
            ],
            temp_dir: "".to_string(),
            default_shell: Shell::Bash,
        };

        let task_names = get_task_names(task_list);

        assert_eq!(task_names, vec!["task1", "task2"]);
    }
}
