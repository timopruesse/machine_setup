pub mod command;
pub mod commands;
pub mod config;
pub mod task_runner;
pub mod utils;

use std::env;

fn main() {
    let config_path = format!(
        "{}/{}",
        env::current_dir().unwrap().to_str().unwrap(),
        "test.yml"
    );

    let run = task_runner::install(&config_path);

    if run.is_err() {
        println!("{}", run.unwrap_err());
    }

    println!("... DONE ...");
}
