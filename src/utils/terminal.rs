use ansi_term::Color::White;
use ergo_fs::PathArc;
use std::env;
use yaml_rust::yaml::Hash;
use yaml_rust::Yaml;

use crate::config::validator::arguments_are_named;
use crate::utils::directory::expand_path;

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
        return Err(String::from("env is not set correctly"));
    }

    Ok(Some(env.unwrap().to_owned()))
}

pub fn set_environment_variables(args: &Yaml) -> Result<(), String> {
    let env = parse_environment_variables(args.to_owned());
    if let Err(err_env) = env {
        return Err(err_env);
    }

    let env = env.unwrap();
    if let Some(env) = env {
        println!("Environment");
        println!("-----------------------------");

        for (key, value) in env {
            let env_name = key.as_str().unwrap();
            let env_value_raw = value.as_str().unwrap();

            let expanded_value =
                expand_path(env_value_raw, false).unwrap_or_else(|_| PathArc::new(env_value_raw));

            let env_value = expanded_value.to_str().unwrap();

            env::set_var(env_name, env_value);
            println!("{}={}", env_name, White.bold().paint(env_value));
        }

        println!("-----------------------------");
    }

    Ok(())
}
