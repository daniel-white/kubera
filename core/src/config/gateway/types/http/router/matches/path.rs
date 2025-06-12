use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpPathMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpPathMatchType::is_default"
    )]
    match_type: HttpPathMatchType,

    #[getset(get = "pub")]
    #[validate(max_length = 1024)]
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

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
