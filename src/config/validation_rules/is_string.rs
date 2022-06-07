use crate::config::{config::ConfigValue, validator::ValidationRule};

pub struct IsString {}

impl ValidationRule for IsString {
    fn validate(&self, input: Option<&ConfigValue>) -> bool {
        let value = input.unwrap_or(&ConfigValue::Invalid);

        if let ConfigValue::String(_) = value {
            return true;
        }

        false
    }

    fn to_string(&self) -> String {
        String::from("argument must be a string")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_fails_when_input_is_not_string() {
        let rule = IsString {};
        let input = ConfigValue::Array(vec![ConfigValue::Integer(1)]);

        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_true_when_value_is_a_string() {
        let rule = IsString {};
        let input = ConfigValue::String(String::from("test"));

        assert!(rule.validate(Some(&input)));
    }
}
