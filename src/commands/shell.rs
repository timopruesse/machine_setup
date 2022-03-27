use std::process::Command;
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    config::{
        validation_rules::{is_array::IsArray, is_string::IsString, one_of::OneOf},
        validator::validate_args,
    },
};

pub struct ShellCommand {}

fn run_shell_command(command: &str) -> Result<(), String> {
    println!("{} ...", command);

    let command = Command::new("sh").arg("-c").arg(command).output();

    if command.is_err() {
        return Err(format!("ERR: {}", command.unwrap_err()));
    }

    let result = command.unwrap();

    if !result.status.success() {
        return Err(format!(
            "{}: {}",
            "test",
            String::from_utf8(result.stderr).unwrap_or(String::from("Error"))
        ));
    }

    println!(
        "{}: {}",
        "test",
        String::from_utf8(result.stdout).unwrap_or(String::from(""))
    );

    return Ok(());
}

impl CommandInterface for ShellCommand {
    fn install(&self, args: Yaml) -> Result<(), String> {
        let validation = validate_args(
            Some(&args),
            vec![&OneOf {
                rules: vec![Box::new(IsArray {}), Box::new(IsString {})],
            }],
        );

        if validation.is_err() {
            return Err(format!("{}", validation.unwrap_err()));
        }

        let commands = if args.is_array() {
            args.as_vec()
                .unwrap()
                .iter()
                .map(|command| command.as_str().unwrap())
                .collect()
        } else {
            vec![args.as_str().unwrap()]
        };

        for command in commands {
            let result = run_shell_command(&command);
            if result.is_err() {
                return Err(format!("{}", result.unwrap_err()));
            }
        }

        return Ok(());
    }

    fn uninstall(&self, args: Yaml) -> Result<(), String> {
        println!("Skipping shell command ...");

        return Ok(());
    }

    fn update(&self, args: Yaml) -> Result<(), String> {
        println!("Skipping shell command ...");

        return Ok(());
    }
}
