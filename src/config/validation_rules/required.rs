use crate::config::{config_value::ConfigValue, validator::ValidationRule};

pub struct Required {}

impl ValidationRule for Required {
    fn validate(&self, input: Option<&ConfigValue>) -> bool {
        if input.is_none() {
            return false;
        }

        let value = input.unwrap_or(&ConfigValue::Invalid);

        if value.is_invalid() || value.is_null() {
            return false;
        }

        return !value.as_str().unwrap().is_empty();
    }

    fn to_string(&self) -> String {
        String::from("argument is required")
    }
}

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_fails_when_required_arg_is_a_bad_value() {
        let rule = Required {};
        let input = ConfigValue::Invalid;
        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_fails_when_required_arg_is_empty() {
        let rule = Required {};
        let input = ConfigValue::String(String::from(""));
        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_fails_when_required_arg_is_null() {
        let rule = Required {};
        let input = ConfigValue::Null;
        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_ok_when_required_arg_is_present() {
        let rule = Required {};
        let input = ConfigValue::String(String::from("hello"));
        assert!(rule.validate(Some(&input)));
    }
}
