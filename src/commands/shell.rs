use std::{fmt::format, process::Command};

use crate::command::CommandInterface;

pub struct ShellCommand {}

impl CommandInterface for ShellCommand {
    fn install(&self, args: yaml_rust::Yaml) -> Result<(), String> {
        let command = Command::new("sh")
            .arg("-c")
            .arg("echo HELLO WORLD")
            .output();

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

    fn uninstall(&self, args: yaml_rust::Yaml) -> Result<(), String> {
        println!("Skipping shell command ...");

        return Ok(());
    }

    fn update(&self, args: yaml_rust::Yaml) -> Result<(), String> {
        println!("Skipping shell command ...");

        return Ok(());
    }
}
