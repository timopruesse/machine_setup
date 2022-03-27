use yaml_rust::Yaml;

use crate::config::validator::ValidationRule;

pub struct OneOf {
    pub rules: Vec<Box<dyn ValidationRule>>,
}

impl ValidationRule for OneOf {
    fn validate(&self, input: Option<&Yaml>) -> bool {
        for rule in &self.rules {
            if rule.validate(input) {
                return true;
            }
        }

        return false;
    }

    fn to_string(&self) -> String {
        return String::from("The argument has to satisfy one of the following rules: ")
            + &self
                .rules
                .iter()
                .map(|rule| rule.to_string())
                .collect::<Vec<String>>()
                .join(", ");
    }
}

// --- tests ---

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn it_returns_true_if_one_rule_is_valid() {
        let rule = OneOf {
            rules: vec![Box::new(Required {}), Box::new(IsArray {})],
        };

        assert!(rule.validate(&Yaml::from_str("foo")));
    }

    #[test]
    fn it_returns_false_if_no_rule_is_valid() {
        let rule = OneOf {
            rules: vec![Box::new(Required {}), Box::new(IsArray {})],
        };

        assert!(!rule.validate(&Yaml::from_str("")));
    }
}
