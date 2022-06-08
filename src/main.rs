use terminal::command::execute_command;

pub mod command;
pub mod commands;
pub mod config;
pub mod task;
pub mod task_runner;
pub mod terminal;
pub mod utils;

fn main() {
    execute_command()
}
