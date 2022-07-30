use crate::config::{config_value::ConfigValue, validator::ValidationRule};

pub struct IsBool {}

impl ValidationRule for IsBool {
    fn validate(&self, input: Option<&ConfigValue>) -> bool {
        if input.is_none() {
            return true;
        }

        let value = input.unwrap_or(&ConfigValue::Invalid);

        if let ConfigValue::Boolean(_) = value {
            return true;
        }

        false
    }

    fn to_string(&self) -> String {
        String::from("argument must be a boolean")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_fails_when_input_is_not_a_boolean() {
        let rule = IsBool {};
        let input = ConfigValue::Array(vec![ConfigValue::Integer(1)]);

        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_true_when_value_is_a_string() {
        let rule = IsBool {};
        let input = ConfigValue::Boolean(true);

        assert!(rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_true_when_value_is_none() {
        let rule = IsBool {};

        assert!(rule.validate(None));
    }
}
