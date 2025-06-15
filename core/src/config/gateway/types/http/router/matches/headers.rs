use crate::config::gateway::types::CaseInsensitiveString;
use getset::Getters;
use http::HeaderName;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::borrow::Cow;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

impl HttpHeaderMatch {
    pub fn exactly<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
        Self {
            match_type: HttpHeaderMatchType::Exact,
            name: HttpHeaderName::new(name),
            value: value.as_ref().to_string(),
        }
    }

    pub fn matches<N: AsRef<str>, P: AsRef<str>>(name: N, pattern: P) -> Self {
        Self {
            match_type: HttpHeaderMatchType::RegularExpression,
            name: HttpHeaderName::new(name),
            value: pattern.as_ref().to_string(),
        }
    }
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

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHeaderName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    CaseInsensitiveString,
);

impl HttpHeaderName {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(CaseInsensitiveString::new(s))
    }
}

impl Into<CaseInsensitiveString> for HttpHeaderName {
    fn into(self) -> CaseInsensitiveString {
        self.0
    }
}

impl Into<String> for HttpHeaderName {
    fn into(self) -> String {
        self.0.to_string()
    }
}

impl JsonSchema for HttpHeaderName {
    fn schema_name() -> Cow<'static, str> {
        Cow::from(stringify!(HttpHeaderName))
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "minLength": 1,
            "maxLength": 256,
            "pattern": "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$"
        })
    }
}

impl From<&HttpHeaderName> for HeaderName {
    fn from(name: &HttpHeaderName) -> Self {
        Self::from_bytes(name.0.to_string().as_bytes()).expect("Invalid header name")
    }
}
