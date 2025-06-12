use derive_builder::Builder;
use getset::Getters;
use http::HeaderName;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpHeaderMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpHeaderMatchType::is_default"
    )]
    match_type: HttpHeaderMatchType,

    #[getset(get = "pub")]
    name: HttpHeaderName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 4096)]
    value: String,
}

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpHeaderMatchType {
    #[default]
    Exact,
    RegularExpression,
}

impl HttpHeaderMatchType {
    fn is_default(&self) -> bool {
        *self == Self::Exact
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpHeaderName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    String,
);

impl From<&HttpHeaderName> for HeaderName {
    fn from(name: &HttpHeaderName) -> Self {
        Self::from_bytes(name.0.as_bytes()).expect("Invalid header name")
    }
}
