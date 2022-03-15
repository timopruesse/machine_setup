#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

pub trait BaseConfig {
    fn read(&self, path: &str) -> Result<TaskList, String>;

    fn next_task(&self) -> Option<Task>;
}
