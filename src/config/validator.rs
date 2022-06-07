use std::collections::HashMap;

use super::config_value::ConfigValue;

pub trait ValidationRule {
    fn validate(&self, input: Option<&ConfigValue>) -> bool;
    fn to_string(&self) -> String;
}

pub fn arguments_are_named(args: Option<&ConfigValue>) -> bool {
    args.unwrap_or(&ConfigValue::Invalid).is_hash()
}

pub fn validate_args(
    args: Option<&ConfigValue>,
    rules: Vec<&impl ValidationRule>,
) -> Result<(), String> {
    if arguments_are_named(args) {
        return Err("Expected positional arguments, got named arguments".to_string());
    }

    for rule in rules {
        if !rule.validate(args) {
            return Err(rule.to_string());
        }
    }

    Ok(())
}

pub fn validate_named_args(
    args: ConfigValue,
    rules: HashMap<String, Vec<&impl ValidationRule>>,
) -> Result<(), String> {
    let named_args = args.as_hash();

    if named_args.is_none() {
        return Err("Expected named arguments, got positional arguments".to_string());
    }

    let named_args = named_args.unwrap();

    for (arg_name, rule_list) in rules {
        let input = named_args.get(arg_name.as_str());

        let result = validate_args(input, rule_list);
        if result.is_err() {
            return Err(format!("{}: {}", arg_name, result.unwrap_err()));
        }
    }

    Ok(())
}

#[cfg(test)]

mod test {
    use crate::config::validation_rules::required::Required;

    use super::*;

    #[test]
    fn it_returns_ok_when_all_rules_pass() {
        let mut rules = HashMap::new();
        rules.insert("foo".to_string(), vec![&Required {}]);

        let mut hash = HashMap::new();
        hash.insert("foo".to_string(), ConfigValue::String("bar".to_string()));

        let args = ConfigValue::Hash(hash);

        assert!(validate_named_args(args, rules).is_ok());
    }

    #[test]
    fn it_returns_an_error_when_a_rule_is_failing() {
        let mut rules = HashMap::new();
        rules.insert("foo".to_string(), vec![&Required {}]);

        let mut hash = HashMap::new();
        hash.insert("foo".to_string(), ConfigValue::Null);

        let args = ConfigValue::Hash(hash);

        assert!(validate_named_args(args, rules).is_err());
    }
}
