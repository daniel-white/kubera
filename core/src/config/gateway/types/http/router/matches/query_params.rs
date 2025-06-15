use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpQueryParamMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpQueryParamMatchType::is_default"
    )]
    match_type: HttpQueryParamMatchType,

    #[getset(get = "pub")]
    name: HttpQueryParamName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 1024)]
    value: String,
}

impl HttpQueryParamMatch {
    pub fn exactly<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
        Self {
            match_type: HttpQueryParamMatchType::Exact,
            name: HttpQueryParamName(name.as_ref().to_string()),
            value: value.as_ref().to_string(),
        }
    }

    pub fn matches<N: AsRef<str>, P: AsRef<str>>(name: N, pattern: P) -> Self {
        Self {
            match_type: HttpQueryParamMatchType::RegularExpression,
            name: HttpQueryParamName(name.as_ref().to_string()),
            value: pattern.as_ref().to_string(),
        }
    }
}

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpQueryParamMatchType {
    #[default]
    Exact,
    RegularExpression,
}

impl HttpQueryParamMatchType {
    fn is_default(&self) -> bool {
        *self == Self::Exact
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpQueryParamName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    #[getset(get = "pub")]
    String,
);
