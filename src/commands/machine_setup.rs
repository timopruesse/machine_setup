use std::collections::HashMap;

use indicatif::ProgressBar;
use tracing::Level;

use crate::{
    command::{CommandConfig, CommandInterface},
    config::{
        config_value::ConfigValue,
        validation_rules::{is_string::IsString, required::Required},
        validator::{validate_named_args, ValidationRule},
    },
    terminal::{
        cli::{Args, SubCommand},
        command::execute_command,
    },
};

pub struct MachineSetupCommand {}

fn execute_config(command: SubCommand, args: ConfigValue) -> Result<(), String> {
    let parameters = args.as_hash();
    if parameters.is_none() {
        return Err(String::from("args is not an object"));
    }
    let parameters = parameters.unwrap();

    let config_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(Required {})];
    let task_rules: Vec<Box<dyn ValidationRule>> = vec![Box::new(IsString {})];

    validate_named_args(
        args.clone(),
        HashMap::from([
            (String::from("config"), config_rules),
            (String::from("task"), task_rules),
        ]),
    )?;

    let config = parameters
        .get("config")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let task = parameters.get("task").unwrap_or(&ConfigValue::Null);
    let task = if task.is_null() {
        None
    } else {
        Some(task.as_str().unwrap().to_string())
    };

    let args = Args {
        command,
        select: false,
        config,
        task,
        debug: false,
        level: Level::WARN,
    };

    execute_command(args);

    Ok(())
}

impl CommandInterface for MachineSetupCommand {
    fn install(
        &self,
        args: ConfigValue,
        _config: &CommandConfig,
        _progress: &ProgressBar,
    ) -> Result<(), String> {
        execute_config(SubCommand::Install, args)
    }

    fn uninstall(
        &self,
        args: ConfigValue,
        _config: &CommandConfig,
        _progress: &ProgressBar,
    ) -> Result<(), String> {
        execute_config(SubCommand::Uninstall, args)
    }

    fn update(
        &self,
        args: ConfigValue,
        _config: &CommandConfig,
        _progress: &ProgressBar,
    ) -> Result<(), String> {
        execute_config(SubCommand::Update, args)
    }
}
