pub struct Task {
    pub name: String,
    pub args: Vec<String>,
}

pub trait BaseConfig {
    fn read(&self, path: &str) -> Result<(), String>;

    fn next_task(&self) -> Option<Task>;
}
