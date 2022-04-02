use crate::config::base_config::TaskList;

pub fn get_task_names(task_list: TaskList) -> Vec<String> {
    let mut task_names = Vec::new();
    for task in task_list.tasks {
        task_names.push(task.name.clone());
    }

    task_names
}
