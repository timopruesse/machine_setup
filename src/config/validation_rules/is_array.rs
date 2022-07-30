use crate::config::{config_value::ConfigValue, validator::ValidationRule};

pub struct IsArray {}

impl ValidationRule for IsArray {
    fn validate(&self, input: Option<&ConfigValue>) -> bool {
        if input.is_none() {
            return true;
        }

        input.unwrap_or(&ConfigValue::Invalid).is_array()
    }

    fn to_string(&self) -> String {
        String::from("argument must be an array")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_fails_when_required_arg_is_not_an_array() {
        let rule = IsArray {};
        let input = ConfigValue::String("foo".to_string());
        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_ok_when_required_arg_is_an_array() {
        let rule = IsArray {};
        let input = ConfigValue::Array(vec![ConfigValue::String("foo".to_string())]);
        assert!(rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_true_when_value_is_none() {
        let rule = IsArray {};

        assert!(rule.validate(None));
    }
}
