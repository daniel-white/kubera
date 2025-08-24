use getset::Getters;
use http::{HeaderName, HeaderValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpHeaderMatch {
    #[getset(get = "pub")]
    kind: HttpHeaderMatchKind,

    #[getset(get = "pub")]
    #[serde(with = "http_serde_ext::header_name")]
    #[schemars(schema_with = "crate::schemars::http_header_name")]
    header: HeaderName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 4096)]
    value: String,
}

impl HttpHeaderMatch {
    pub fn exactly<H: Into<HeaderName>, V: Into<HeaderValue>>(header: H, value: V) -> Self {
        let value: HeaderValue = value.into();
        Self {
            kind: HttpHeaderMatchKind::Exact,
            header: header.into(),
            value: value.to_str().expect("Invalid header value").to_string(),
        }
    }

    pub fn matches<H: Into<HeaderName>, P: AsRef<str>>(header: H, pattern: P) -> Self {
        Self {
            kind: HttpHeaderMatchKind::RegularExpression,
            header: header.into(),
            value: pattern.as_ref().to_string(),
        }
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpHeaderMatchKind {
    Exact,
    RegularExpression,
}
