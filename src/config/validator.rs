use std::collections::HashMap;
use yaml_rust::Yaml;

pub trait ValidationRule {
    fn validate(&self, input: Option<&Yaml>) -> bool;
    fn to_string(&self) -> String;
}

pub fn validate_args(
    args: Yaml,
    rules: HashMap<String, Vec<&impl ValidationRule>>,
) -> Result<(), String> {
    if args.is_array() {
        println!("Not validating array: {:?}", args);
        return Ok(());
    }

    for (arg_name, ruleList) in rules {
        let input = args.as_hash().unwrap().get(&Yaml::String(arg_name.clone()));

        for rule in ruleList {
            if !rule.validate(input) {
                return Err(format!("{}: {}", arg_name, rule.to_string()));
            }
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
        let mut args = Yaml::Hash(LinkedHashMap::new());
        args.as_hash().insert("arg1".to_string());

        let rules = HashMap::new();
        rules.insert(String::from("arg1"), vec![Box::new(Required {})]);

        assert!(validate_args(args, rules).is_ok());
    }

    #[test]
    fn it_returns_an_error_when_a_rule_is_failing() {
        let args = Yaml::Hash(LinkedHashMap::new());

        let rules = HashMap::new();
        rules.insert(String::from("arg1"), vec![Box::new(Required {})]);

        assert!(validate_args(args, rules).is_err());
    }
}
