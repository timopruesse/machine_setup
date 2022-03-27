use yaml_rust::Yaml;

use crate::config::validator::ValidationRule;

pub struct Required {}

impl ValidationRule for Required {
    fn validate(&self, input: Option<&Yaml>) -> bool {
        if input.is_none() {
            return false;
        }

        let value = input.unwrap();

        if value.is_badvalue() || value.is_null() {
            return false;
        }

        return !value.as_str().unwrap().is_empty();
    }

    fn to_string(&self) -> String {
        return String::from("argument is required");
    }
}

// --- tests ---

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_fails_when_required_arg_is_a_bad_value() {
        let rule = Required {};
        let input = Yaml::BadValue;
        assert!(!rule.validate(&input));
    }

    #[test]
    fn it_fails_when_required_arg_is_empty() {
        let rule = Required {};
        let input = Yaml::String(String::from(""));
        assert!(!rule.validate(&input));
    }

    #[test]
    fn it_fails_when_required_arg_is_null() {
        let rule = Required {};
        let input = Yaml::Null;
        assert!(!rule.validate(&input));
    }

    #[test]
    fn it_returns_ok_when_required_arg_is_present() {
        let rule = Required {};
        let input = Yaml::String(String::from("hello"));
        assert!(rule.validate(&input));
    }
}
