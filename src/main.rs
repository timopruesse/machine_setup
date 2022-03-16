pub mod command;
pub mod commands;
pub mod config;

// use commands::copy::copy_dir;
// use home::home_dir;

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
            let result = resolved_command.execute(command.args);

            println!("{} {:?}", command.name, result);
        }
    }

    // let home = home_dir().unwrap();
    // let home_path = home.to_str().unwrap();
    // let src_path = format!("{}/install-wsl", home_path);
    // let dst_path = format!("{}/install-wsl2", home_path);

    // copy_dir(&src_path, &dst_path).unwrap();
}
