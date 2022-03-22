pub mod command;
pub mod commands;
pub mod config;
pub mod utils;

use std::env;

use config::base_config::BaseConfig;
use config::yaml_config::YamlConfig;

use crate::command::get_command;

fn main() {
    let config = YamlConfig {};
    let result = config.read(&format!(
        "{}/{}",
        env::current_dir().unwrap().to_str().unwrap(),
        "test.yml"
    ));

    for task in result.unwrap().tasks {
        let commands = task.commands;
        for command in commands {
            let resolved_command = get_command(&command.name).unwrap();
            let result = resolved_command.install(command.args);

            println!("{} {:?}", command.name, result);
        }
    }
}
