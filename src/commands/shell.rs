use core::fmt;
use ergo_fs::PathArc;
use std::{collections::HashMap, env, process::Command};
use yaml_rust::yaml::Hash;
use yaml_rust::Yaml;

use crate::{
    command::CommandInterface,
    config::{
        validation_rules::{is_array::IsArray, is_string::IsString, one_of::OneOf},
        validator::{arguments_are_named, validate_args, validate_named_args},
    },
    utils::directory::expand_path,
};

pub struct ShellCommand {}

fn run_shell_command(command_name: &str) -> Result<String, String> {
    println!("{} ...", command_name);

    let command = Command::new("sh").arg("-c").arg(command_name).output();

    if let Err(err_command) = command {
        return Err(format!("ERR: {}", err_command));
    }

    let output = command.unwrap();

    let mut stdout = String::from_utf8(output.stdout).unwrap_or_else(|_| String::from("OK"));
    if stdout.is_empty() {
        stdout = String::from("OK");
    }

    if !output.status.success() {
        let error_msg = String::from_utf8(output.stderr).unwrap_or_else(|e| e.to_string());

        return Err(format!(
            "{}: {}",
            command_name,
            if error_msg.is_empty() {
                stdout
            } else {
                error_msg
            }
        ));
    }

    Ok(format!("{}: {}", command_name, stdout))
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

        if let Err(err_validation) = validation {
            return Err(format!("ERR: {}", err_validation));
        }

        return Ok(get_commands_from_yaml(
            args.as_hash()
                .unwrap()
                .get(&Yaml::String(method_name))
                .unwrap()
                .to_owned(),
        ));
    }

    let validation = validate_args(Some(&args), rules);
    if let Err(err_validation) = validation {
        return Err(err_validation);
    }

    Ok(get_commands_from_yaml(args))
}

fn parse_environment_variables(args: Yaml) -> Result<Option<Hash>, String> {
    if !arguments_are_named(Some(&args)) {
        return Ok(None);
    }

    let hash = args.as_hash().unwrap();
    if !hash.contains_key(&Yaml::String(String::from("env"))) {
        return Ok(None);
    }

    let env = hash
        .get(&Yaml::String(String::from("env")))
        .unwrap()
        .as_hash();

    if env.is_none() {
        return Err(String::from("ERR: env is not set correctly"));
    }

    Ok(Some(env.unwrap().to_owned()))
}

fn set_environment_variables(args: &Yaml) -> Result<(), String> {
    let env = parse_environment_variables(args.to_owned());
    if let Err(err_env) = env {
        return Err(err_env);
    }

    let env = env.unwrap();
    if let Some(env) = env {
        println!("Setting environment...");
        println!("-------------------");

        for (key, value) in env {
            let env_name = key.as_str().unwrap();
            let env_value_raw = value.as_str().unwrap();

            let expanded_value =
                expand_path(env_value_raw, false).unwrap_or_else(|_| PathArc::new(env_value_raw));

            let env_value = expanded_value.to_str().unwrap();

            env::set_var(env_name, env_value);
            println!("{}={}", env_name, env_value);
        }

        println!("-------------------");
    }

    Ok(())
}

impl CommandInterface for ShellCommand {
    fn install(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args.clone(), ShellMethod::Install);

        if let Err(err_commands) = commands {
            return Err(err_commands);
        }

        if let Err(err_env) = set_environment_variables(&args) {
            return Err(err_env);
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if let Err(err_result) = result {
                return Err(err_result);
            }

            result.unwrap().split('\n').for_each(|line| {
                println!("{}", line);
            });
        }

        Ok(())
    }

    fn uninstall(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args.clone(), ShellMethod::Uninstall);

        if let Err(err_commands) = commands {
            return Err(err_commands);
        }

        if let Err(err_env) = set_environment_variables(&args) {
            return Err(err_env);
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if let Err(err_result) = result {
                return Err(err_result);
            }

            result.unwrap().split('\n').for_each(|line| {
                println!("{}", line);
            });
        }

        Ok(())
    }

    fn update(&self, args: Yaml) -> Result<(), String> {
        let commands = get_commands(args.clone(), ShellMethod::Update);

        if let Err(err_commands) = commands {
            return Err(err_commands);
        }

        if let Err(err_env) = set_environment_variables(&args) {
            return Err(err_env);
        }

        for command in commands.unwrap() {
            let result = run_shell_command(&command);
            if let Err(err_result) = result {
                return Err(err_result);
            }

            result.unwrap().split('\n').for_each(|line| {
                println!("{}", line);
            });
        }

        Ok(())
    }
}

// -- tests --

#[cfg(test)]
mod test {
    // TODO: Add tests
}
