use crate::config::{config::ConfigValue, validator::ValidationRule};

pub struct OneOf {
    pub rules: Vec<Box<dyn ValidationRule>>,
}

impl ValidationRule for OneOf {
    fn validate(&self, input: Option<&ConfigValue>) -> bool {
        for rule in &self.rules {
            if rule.validate(input) {
                return true;
            }
        }

        false
    }

    fn to_string(&self) -> String {
        return String::from("OneOf: ")
            + &self
                .rules
                .iter()
                .map(|rule| rule.to_string())
                .collect::<Vec<String>>()
                .join(" | ");
    }
}

#[cfg(test)]

mod test {
    use crate::config::validation_rules::{is_array::IsArray, required::Required};

    use super::*;

    #[test]
    fn it_returns_true_if_one_rule_is_valid() {
        let rule = OneOf {
            rules: vec![Box::new(Required {}), Box::new(IsArray {})],
        };

        assert!(rule.validate(Some(&ConfigValue::String(String::from("foo")))));
    }

    #[test]
    fn it_returns_false_if_no_rule_is_valid() {
        let rule = OneOf {
            rules: vec![Box::new(Required {}), Box::new(IsArray {})],
        };

        assert!(!rule.validate(Some(&ConfigValue::String(String::from("")))));
    }
}
