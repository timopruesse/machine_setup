use ansi_term::Color::White;
use ergo_fs::PathBuf;
use std::env;

use crate::config::config_value::ConfigValue;
use crate::config::validator::arguments_are_named;
use crate::utils::directory::expand_path;

fn parse_environment_variables(args: ConfigValue) -> Result<Option<ConfigValue>, String> {
    if !arguments_are_named(Some(&args)) {
        return Ok(None);
    }

    let hash = args.as_hash().unwrap();
    if !hash.contains_key("env") {
        return Ok(None);
    }

    if let Some(env) = hash.get("env") {
        return Ok(Some(env.to_owned()));
    }

    Err(String::from("env is not set correctly"))
}

pub fn set_environment_variables(args: &ConfigValue) -> Result<(), String> {
    let env = parse_environment_variables(args.to_owned())?;

    if let Some(env) = env {
        println!("Environment");
        println!("-----------------------------");

        if !env.is_hash() {
            return Err(String::from("Environment needs to be defined as a map"));
        }

        for (key, value) in env.as_hash().unwrap() {
            let env_value_raw = value.as_str().unwrap();

            let expanded_value =
                expand_path(env_value_raw, false).unwrap_or_else(|_| PathBuf::from(env_value_raw));

            let env_value = expanded_value.to_str().unwrap();

            env::set_var(key, env_value);
            println!("{}={}", key, White.bold().paint(env_value));
        }

        println!("-----------------------------");
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn it_sets_environment_variables_correctly() {
        let mut env = HashMap::new();
        env.insert(
            String::from("TEST_1"),
            ConfigValue::String(String::from("value_one")),
        );
        env.insert(
            String::from("TEST_2"),
            ConfigValue::String(String::from("value_two")),
        );

        let mut args = HashMap::new();
        args.insert("env".to_string(), ConfigValue::Hash(env));

        set_environment_variables(&ConfigValue::Hash(args)).unwrap();

        assert_eq!(env::var("TEST_1").unwrap(), "value_one");
        assert_eq!(env::var("TEST_2").unwrap(), "value_two");
    }
}
