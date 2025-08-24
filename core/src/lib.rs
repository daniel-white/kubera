#![warn(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented
)]
#![allow(
    clippy::needless_continue,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::struct_field_names,
    mismatched_lifetime_syntaxes
)]

pub mod config;
pub mod crypto;
pub mod gateways;
pub mod http;
pub mod instrumentation;
pub mod io;
pub mod ipc;
pub mod net;
mod schemars;
pub mod sync;
pub mod task;
pub mod utils;

pub use crate::sync::macros::*;

use serde::{Deserialize, Serialize};
use serde_valid::export::regex::Regex;
use serde_valid::{
    MaxLengthError, MinLengthError, PatternError, Validate, ValidateMaxLength, ValidateMinLength,
    ValidatePattern,
};
use std::fmt::{Display, Formatter};
use unicase::UniCase;

#[derive(Validate, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveString(UniCase<String>);

impl CaseInsensitiveString {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(UniCase::from(s.as_ref()))
    }

    pub fn ends_with(&self, suffix: &Self) -> bool {
        let self_len = self.0.len();
        let suffix_len = suffix.0.len();
        if self_len < suffix_len {
            return false;
        }
        let self_suffix = Self::new(&self.0[self_len - suffix_len..]);
        self_suffix == *suffix
    }
}

impl Display for CaseInsensitiveString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for CaseInsensitiveString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CaseInsensitiveString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl ValidateMinLength for CaseInsensitiveString {
    fn validate_min_length(&self, min: usize) -> Result<(), MinLengthError> {
        self.0.validate_min_length(min)
    }
}

impl ValidateMaxLength for CaseInsensitiveString {
    fn validate_max_length(&self, max: usize) -> Result<(), MaxLengthError> {
        self.0.validate_max_length(max)
    }
}

impl ValidatePattern for CaseInsensitiveString {
    fn validate_pattern(&self, pattern: &Regex) -> Result<(), PatternError> {
        self.0.validate_pattern(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::assert_ok;
    use proptest::prelude::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn sample_string() -> String {
        "Hello World".to_string()
    }

    #[fixture]
    fn case_insensitive_string() -> CaseInsensitiveString {
        CaseInsensitiveString::new("Hello World")
    }

    #[test]
    fn test_case_insensitive_string_creation() {
        let cis = CaseInsensitiveString::new("test");
        assert_eq!(cis.to_string(), "test");
    }

    #[test]
    fn test_case_insensitive_string_equality() {
        let cis1 = CaseInsensitiveString::new("Hello");
        let cis2 = CaseInsensitiveString::new("hello");
        let cis3 = CaseInsensitiveString::new("HELLO");

        assert_eq!(cis1, cis2);
        assert_eq!(cis2, cis3);
        assert_eq!(cis1, cis3);
    }

    #[test]
    fn test_case_insensitive_string_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let key1 = CaseInsensitiveString::new("KEY");
        let key2 = CaseInsensitiveString::new("key");

        map.insert(key1, "value1");
        map.insert(key2, "value2");

        // Should only have one entry due to case insensitivity
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&CaseInsensitiveString::new("Key")), Some(&"value2"));
    }

    #[rstest]
    #[case("hello", "lo", true)]
    #[case("HELLO", "lo", true)]
    #[case("hello", "LO", true)]
    #[case("hello", "world", false)]
    #[case("", "", true)]
    fn test_ends_with(#[case] input: &str, #[case] suffix: &str, #[case] expected: bool) {
        let cis = CaseInsensitiveString::new(input);
        let suffix_cis = CaseInsensitiveString::new(suffix);
        assert_eq!(cis.ends_with(&suffix_cis), expected);
    }

    #[test]
    fn test_serialization() {
        let cis = CaseInsensitiveString::new("Test Value");
        let serialized = assert_ok!(serde_json::to_string(&cis));
        assert_eq!(serialized, "\"Test Value\"");

        let deserialized: CaseInsensitiveString = assert_ok!(serde_json::from_str(&serialized));
        assert_eq!(cis, deserialized);
    }

    #[test]
    fn test_validation() {
        use serde_valid::Validate;

        let cis = CaseInsensitiveString::new("test");
        assert!(cis.validate().is_ok());
    }

    proptest! {
        #[test]
        fn test_case_insensitive_string_properties(s in "[a-zA-Z0-9]{1,10}") {
            let cis1 = CaseInsensitiveString::new(&s);
            let cis2 = CaseInsensitiveString::new(s.to_lowercase());
            let cis3 = CaseInsensitiveString::new(s.to_uppercase());

            // All variations should be equal
            prop_assert_eq!(cis1.clone(), cis2.clone());
            prop_assert_eq!(cis2.clone(), cis3.clone());

            // String representation should preserve original case for the first one
            prop_assert_eq!(cis1.to_string(), s);
        }

        #[test]
        fn test_ends_with_property(s in "[a-zA-Z0-9]{1,10}", suffix in "[a-zA-Z0-9]{1,5}") {
            let full_string = format!("{s}{suffix}");
            let cis = CaseInsensitiveString::new(&full_string);
            let suffix_cis = CaseInsensitiveString::new(&suffix);

            prop_assert!(cis.ends_with(&suffix_cis));
        }
    }
}
