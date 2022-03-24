use crate::{
    command::get_command,
    config::{base_config::BaseConfig, yaml_config::YamlConfig},
};

enum TaskRunnerMode {
    Install,
    Update,
    Uninstall,
}

fn get_config_handler(config_path: &str) -> Result<Box<dyn BaseConfig>, String> {
    let file_ending = config_path.split('.').last().unwrap();

    return match file_ending {
        file_ending if file_ending == "yml" || file_ending == "yaml" => Ok(Box::new(YamlConfig {})),
        _ => Err(String::from("Unsupported config file format")),
    };
}

fn run(config_path: &str, mode: TaskRunnerMode) -> Result<(), String> {
    let config = get_config_handler(config_path);
    if config.is_err() {
        return Err(config.err().unwrap());
    }
    let config = config.unwrap();

    let result = config.read(config_path);

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    for task in result.unwrap().tasks {
        println!("Running task \"{}\" ...", task.name);

        let commands = task.commands;
        for command in commands {
            let resolved_command = get_command(&command.name).unwrap();
            let result = resolved_command.install(command.args);

            if result.is_ok() {
                println!("OK: {}", command.name);
            } else {
                println!("ERR: {}", command.name);
                println!("{:?}", result);
            }
        }
    }

    return Ok(());
}

pub fn install(config_path: &str) -> Result<(), String> {
    println!("Installing ...");

    return run(config_path, TaskRunnerMode::Install);
}

pub fn update(config_path: &str) -> Result<(), String> {
    println!("Updating ...");

    return run(config_path, TaskRunnerMode::Update);
}

pub fn uninstall(config_path: &str) -> Result<(), String> {
    println!("Uninstalling ...");

    return run(config_path, TaskRunnerMode::Uninstall);
}
