use std::collections::HashMap;
use yaml_rust::Yaml;

pub trait ValidationRule {
    fn validate(&self, input: Option<&Yaml>) -> bool;
    fn to_string(&self) -> String;
}

pub fn validate_args(args: Option<&Yaml>, rules: Vec<&impl ValidationRule>) -> Result<(), String> {
    let is_named = !args.unwrap_or(&Yaml::BadValue).as_hash().is_none();
    if is_named {
        return Err(format!(
            "Expected positional arguments, got named arguments"
        ));
    }

    for rule in rules {
        if !rule.validate(args) {
            return Err(format!("{}", rule.to_string()));
        }
    }

    return Ok(());
}

pub fn validate_named_args(
    args: Yaml,
    rules: HashMap<String, Vec<&impl ValidationRule>>,
) -> Result<(), String> {
    let named_args = args.as_hash();
    if named_args.is_none() {
        return Err(format!(
            "Expected named arguments, got positional arguments"
        ));
    }

    let named_args = named_args.unwrap();

    for (arg_name, rule_list) in rules {
        let input = named_args.get(&Yaml::String(arg_name.clone()));

        let result = validate_args(input, rule_list);
        if result.is_err() {
            return Err(format!("{}: {}", arg_name, result.unwrap_err()));
        }
    }

    return Ok(());
}

// --- tests ---

#[cfg(test)]

mod test {
    use crate::config::validation_rules::required::Required;

    use super::*;

    #[test]
    fn it_returns_ok_when_all_rules_pass() {
        let mut rules = HashMap::new();
        rules.insert("foo".to_string(), vec![&Required {}]);

        let args = Hash::new();
        args.insert(
            Yaml::String("foo".to_string()),
            Yaml::String("bar".to_string()),
        );

        assert!(validate_named_args(args, rules).is_ok());
    }

    #[test]
    fn it_returns_an_error_when_a_rule_is_failing() {
        let mut rules = HashMap::new();
        rules.insert("foo".to_string(), vec![&Required {}]);

        let args = Hash::new();
        args.insert(
            Yaml::String("foo".to_string()),
            Yaml::String("".to_string()),
        );

        assert!(validate_named_args(args, rules).is_err());
    }
}
