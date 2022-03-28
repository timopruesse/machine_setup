use core::fmt;
use std::{collections::HashMap, process::Command};
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    config::{
        validation_rules::{is_array::IsArray, is_string::IsString, one_of::OneOf},
        validator::{arguments_are_named, validate_args, validate_named_args},
    },
};

pub struct ShellCommand {}

fn run_shell_command(command_name: &str) -> Result<(), String> {
    println!("{} ...", command_name);

    let command = Command::new("sh").arg("-c").arg(command_name).output();

    if command.is_err() {
        return Err(format!("ERR: {}", command.unwrap_err()));
    }

    let output = command.unwrap();

    if !output.status.success() {
        let error_msg = String::from_utf8(output.stderr).unwrap_or(String::from("Error"));

        return Err(format!(
            "{}: {}",
            command_name,
            if error_msg.is_empty() {
                String::from("Error")
            } else {
                error_msg
            }
        ));
    }

    let stdout = String::from_utf8(output.stdout).unwrap_or(String::from("OK"));

    println!(
        "{}: {}",
        command_name,
        if stdout.is_empty() {
            String::from("OK")
        } else {
            stdout
        }
    );

    return Ok(());
}

fn get_commands_from_yaml(args: Yaml) -> Vec<String> {
    return if args.is_array() {
        args.as_vec()
            .unwrap()
            .iter()
            .map(|command| command.as_str().unwrap().to_string())
            .collect()
    } else {
        vec![args.as_str().unwrap().to_string()]
    };
}

#[derive(Debug)]
enum ShellMethod {
    Install,
    Update,
    Uninstall,
}

impl fmt::Display for ShellMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let method = match self {
            ShellMethod::Install => "install",
            ShellMethod::Update => "update",
            ShellMethod::Uninstall => "uninstall",
        };

        write!(f, "{}", method)
    }
}

fn get_commands(args: Yaml, method: ShellMethod) -> Result<Vec<String>, String> {
    let is_str_or_array = OneOf {
        rules: vec![Box::new(IsArray {}), Box::new(IsString {})],
    };
    let rules = vec![&is_str_or_array];

    let method_name = method.to_string();

    if arguments_are_named(Some(&args)) {
        let validation =
            validate_named_args(args.clone(), HashMap::from([(method_name.clone(), rules)]));
        if validation.is_err() {
            return Err(format!("ERR: {}", validation.unwrap_err()));
        }

        return Ok(get_commands_from_yaml(
            args.clone()
                .as_hash()
                .unwrap()
                .get(&Yaml::String(method_name))
                .unwrap()
                .to_owned(),
        ));
    }

    let validation = validate_args(Some(&args), rules);
    if validation.is_err() {
        return Err(format!("ERR: {}", validation.unwrap_err()));
    }

    return Ok(get_commands_from_yaml(args.clone()));
}

impl CommandInterface for ShellCommand {
    fn install(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args, ShellMethod::Install);

        if commands.is_err() {
            return Err(commands.unwrap_err());
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if result.is_err() {
                return Err(format!("{}", result.unwrap_err()));
            }
        }

        return Ok(());
    }

    fn uninstall(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args, ShellMethod::Uninstall);

        if commands.is_err() {
            return Err(commands.unwrap_err());
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if result.is_err() {
                return Err(format!("{}", result.unwrap_err()));
            }
        }

        return Ok(());
    }

    fn update(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args, ShellMethod::Update);

        if commands.is_err() {
            return Err(commands.unwrap_err());
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if result.is_err() {
                return Err(format!("{}", result.unwrap_err()));
            }
        }

        return Ok(());
    }
}
