use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use typed_builder::TypedBuilder;

#[derive(
    Validate,
    TypedBuilder,
    Getters,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Hash,
    JsonSchema,
)]
pub struct HttpPathMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpPathMatchType::is_default"
    )]
    #[builder(setter(into))]
    match_type: HttpPathMatchType,

    #[getset(get = "pub")]
    #[validate(max_length = 1024)]
    #[builder(setter(into))]
    value: String,
}

impl Default for HttpPathMatch {
    fn default() -> Self {
        HttpPathMatch {
            match_type: HttpPathMatchType::Prefix,
            value: "/".to_string(),
        }
    }
}

impl HttpPathMatch {
    pub fn exactly<S: AsRef<str>>(path: S) -> Self {
        Self {
            match_type: HttpPathMatchType::Exact,
            value: path.as_ref().to_string(),
        }
    }

    pub fn with_prefix<S: AsRef<str>>(prefix: S) -> Self {
        Self {
            match_type: HttpPathMatchType::Prefix,
            value: prefix.as_ref().to_string(),
        }
    }

    pub fn matching<S: AsRef<str>>(pattern: S) -> Self {
        Self {
            match_type: HttpPathMatchType::RegularExpression,
            value: pattern.as_ref().to_string(),
        }
    }

    #[must_use]
    pub fn is_default(&self) -> bool {
        self.match_type.is_default() && self.value == "/"
    }
}

#[derive(
    Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
pub enum HttpPathMatchType {
    Exact,
    #[default]
    Prefix,
    RegularExpression,
}

impl HttpPathMatchType {
    fn is_default(&self) -> bool {
        *self == Self::Prefix
    }
}
