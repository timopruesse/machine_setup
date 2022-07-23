use std::{
    collections::HashMap,
    fs::remove_file,
    process::{Command, Stdio},
    str::FromStr,
};

use crate::{
    command::{CommandConfig, CommandInterface},
    config::{
        config_value::ConfigValue,
        validation_rules::{is_array::IsArray, is_string::IsString, one_of::OneOf},
        validator::{arguments_are_named, validate_args, validate_named_args},
    },
    task_runner::TaskRunnerMode,
    utils::{
        shell::{create_script_file, Shell},
        terminal::set_environment_variables,
    },
};

pub struct RunCommand {}

fn get_commands_from_yaml(args: ConfigValue) -> Vec<String> {
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

fn get_commands(args: ConfigValue, mode: TaskRunnerMode) -> Result<Vec<String>, String> {
    let is_str_or_array = OneOf {
        rules: vec![Box::new(IsArray {}), Box::new(IsString {})],
    };
    let rules = vec![&is_str_or_array];

    let method_name = mode.to_string();

    if arguments_are_named(Some(&args)) {
        validate_named_args(args.clone(), HashMap::from([(method_name.clone(), rules)]))?;

        return Ok(get_commands_from_yaml(
            args.as_hash()
                .unwrap()
                .get(&method_name)
                .unwrap()
                .to_owned(),
        ));
    }

    validate_args(Some(&args), rules)?;

    Ok(get_commands_from_yaml(args))
}

fn run_commands(
    commands: &ConfigValue,
    shell: &str,
    mode: TaskRunnerMode,
    temp_dir: &str,
) -> Result<String, String> {
    let parsed_commands = get_commands(commands.clone(), mode)?;
    let temp_script = create_script_file(
        Shell::from_str(shell).unwrap_or(Shell::Bash),
        parsed_commands,
        temp_dir,
    )?;

    let command = Command::new(shell)
        .arg("-c")
        .arg(&temp_script)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output();

    remove_file(temp_script).ok();

    if let Err(err_command) = command {
        return Err(err_command.to_string());
    }

    let output = command.unwrap();

    let mut stdout = String::from_utf8(output.stdout).unwrap_or_else(|_| String::from("OK"));
    if stdout.is_empty() {
        stdout = String::from("\n");
    }

    if !output.status.success() {
        let error_msg = String::from_utf8(output.stderr).unwrap_or_else(|e| e.to_string());

        return Err(if error_msg.is_empty() {
            stdout
        } else {
            error_msg
        });
    }

    Ok(stdout)
}

fn run_task(mode: TaskRunnerMode, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
    let parameters = args.as_hash();
    if parameters.is_none() {
        return Err(String::from("args is not an object"));
    }
    let parameters = parameters.unwrap();

    let param_commands = parameters.get("commands");
    if param_commands.is_none() {
        return Err(String::from("\"commands\" is missing in args"));
    }
    let param_commands = param_commands.unwrap();

    let default_shell = ConfigValue::String(config.default_shell.to_string());
    let param_shell = parameters
        .get("shell")
        .unwrap_or(&default_shell)
        .as_str()
        .unwrap();

    set_environment_variables(&args)?;

    let result = run_commands(param_commands, param_shell, mode, &config.temp_dir)?;

    result.split('\n').for_each(|line| println!("{}", line));

    Ok(())
}

impl CommandInterface for RunCommand {
    fn install(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        run_task(TaskRunnerMode::Install, args, config)
    }

    fn uninstall(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        run_task(TaskRunnerMode::Uninstall, args, config)
    }

    fn update(&self, args: ConfigValue, config: &CommandConfig) -> Result<(), String> {
        run_task(TaskRunnerMode::Update, args, config)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_gets_command_from_string() {
        let command = "echo hello";

        let commands = get_commands(
            ConfigValue::String(command.to_string()),
            TaskRunnerMode::Install,
        );

        assert!(commands.is_ok());
        assert_eq!(vec![command.to_string()], commands.unwrap());
    }

    #[test]
    fn it_gets_commands_from_array() {
        let commands = ConfigValue::Array(vec![
            ConfigValue::String(String::from("command1")),
            ConfigValue::String(String::from("command2")),
        ]);

        let commands = get_commands(commands, TaskRunnerMode::Install);
        assert!(commands.is_ok());
        assert_eq!(
            commands.unwrap(),
            vec![String::from("command1"), String::from("command2")]
        );
    }

    #[test]
    fn it_gets_install_commands() {
        let mut commands = HashMap::new();
        commands.insert(
            "install".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("command1".to_string()),
                ConfigValue::String("command2".to_string()),
            ]),
        );

        let commands = get_commands(ConfigValue::Hash(commands.clone()), TaskRunnerMode::Install);
        assert!(commands.is_ok());
        assert_eq!(
            commands.unwrap(),
            vec![String::from("command1"), String::from("command2")]
        );
    }

    #[test]
    fn it_gets_install_command_string() {
        let mut commands = HashMap::new();
        commands.insert(
            "install".to_string(),
            ConfigValue::String(String::from("command1")),
        );

        let commands = get_commands(ConfigValue::Hash(commands.clone()), TaskRunnerMode::Install);
        assert!(commands.is_ok());
        assert_eq!(commands.unwrap(), vec![String::from("command1")]);
    }

    #[test]
    fn it_gets_uninstall_commands() {
        let mut commands = HashMap::new();
        commands.insert(
            "uninstall".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("command1".to_string()),
                ConfigValue::String("command2".to_string()),
            ]),
        );

        let commands = get_commands(
            ConfigValue::Hash(commands.clone()),
            TaskRunnerMode::Uninstall,
        );
        assert!(commands.is_ok());
        assert_eq!(
            commands.unwrap(),
            vec![String::from("command1"), String::from("command2")]
        );
    }

    #[test]
    fn it_gets_uninstall_command_string() {
        let mut commands = HashMap::new();
        commands.insert(
            "uninstall".to_string(),
            ConfigValue::String(String::from("command1")),
        );

        let commands = get_commands(
            ConfigValue::Hash(commands.clone()),
            TaskRunnerMode::Uninstall,
        );
        assert!(commands.is_ok());
        assert_eq!(commands.unwrap(), vec![String::from("command1")]);
    }

    #[test]
    fn it_gets_update_commands() {
        let mut commands = HashMap::new();
        commands.insert(
            "update".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("command1".to_string()),
                ConfigValue::String("command2".to_string()),
            ]),
        );

        let commands = get_commands(ConfigValue::Hash(commands.clone()), TaskRunnerMode::Update);
        assert!(commands.is_ok());
        assert_eq!(
            commands.unwrap(),
            vec![String::from("command1"), String::from("command2")]
        );
    }

    #[test]
    fn it_gets_update_command_string() {
        let mut commands = HashMap::new();
        commands.insert(
            "update".to_string(),
            ConfigValue::String(String::from("command1")),
        );

        let commands = get_commands(ConfigValue::Hash(commands.clone()), TaskRunnerMode::Update);
        assert!(commands.is_ok());
        assert_eq!(commands.unwrap(), vec![String::from("command1")]);
    }

    #[test]
    fn it_fails_when_method_is_not_defined() {
        let mut commands = HashMap::new();
        commands.insert(
            "invalid".to_string(),
            ConfigValue::String(String::from("command1")),
        );

        let commands = get_commands(ConfigValue::Hash(commands.clone()), TaskRunnerMode::Install);
        assert!(commands.is_err());
        assert_eq!(
            commands.unwrap_err(),
            String::from("install: OneOf: argument must be an array | argument must be a string")
        );
    }
}
