use serde::{Deserialize, Serialize};
use serde_valid::export::regex::Regex;
use serde_valid::{
    MaxLengthError, MinLengthError, PatternError, Validate, ValidateMaxLength, ValidateMinLength,
    ValidatePattern,
};
use std::fmt::{Display, Formatter};
use unicase::UniCase;

pub mod config;
pub mod io;
pub mod net;
pub mod sync;

#[derive(Validate, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveString(UniCase<String>);

impl CaseInsensitiveString {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(UniCase::from(s.as_ref()))
    }

    pub fn ends_with(&self, suffix: &Self) -> bool {
        self.0.ends_with(&suffix.0.as_str())
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
