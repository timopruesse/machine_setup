use yaml_rust::Yaml;

use crate::config::validator::ValidationRule;

pub struct IsArray {}

impl ValidationRule for IsArray {
    fn validate(&self, input: Option<&Yaml>) -> bool {
        input.unwrap_or(&Yaml::BadValue).is_array()
    }

    fn to_string(&self) -> String {
        String::from("argument must be an array")
    }
}

// --- tests ---

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_fails_when_required_arg_is_not_an_array() {
        let rule = IsArray {};
        let input = Yaml::String("foo".to_string());
        assert!(!rule.validate(Some(&input)));
    }

    #[test]
    fn it_returns_ok_when_required_arg_is_an_array() {
        let rule = IsArray {};
        let input = Yaml::Array(vec![Yaml::String("foo".to_string())]);
        assert!(rule.validate(Some(&input)));
    }
}
