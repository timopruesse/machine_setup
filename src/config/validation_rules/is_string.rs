use yaml_rust::Yaml;

use crate::config::validator::ValidationRule;

pub struct IsString {}

impl ValidationRule for IsString {
    fn validate(&self, input: Option<&Yaml>) -> bool {
        let value = input.unwrap_or(&Yaml::BadValue);

        if let Yaml::String(_) = value {
            return true;
        }

        false
    }

    fn to_string(&self) -> String {
        String::from("argument must be a string")
    }
}

// --- tests ---

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_fails_when_input_is_not_string() {
        let rule = IsString {};
        let input = Yaml::Array(vec![Yaml::Integer(1)]);

        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_true_when_value_is_a_string() {
        let rule = IsString {};
        let input = Yaml::String(String::from("test"));

        assert!(rule.validate(Some(&input)));
    }
}
