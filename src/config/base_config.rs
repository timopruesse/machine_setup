use yaml_rust::Yaml;

#[derive(Debug)]
pub struct Command {
    pub name: String,
    pub args: Yaml,
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub commands: Vec<Command>,
}

#[derive(Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

pub trait BaseConfig {
    fn read(&self, path: &str) -> Result<TaskList, String>;
}
