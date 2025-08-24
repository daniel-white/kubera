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
#[serde(rename_all = "camelCase")]
pub struct HttpQueryParamMatch {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: HttpQueryParamMatchKind,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    name: HttpQueryParamName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 1024)]
    #[builder(setter(into))]
    value: String,
}

impl HttpQueryParamMatch {
    pub fn exactly<N: Into<HttpQueryParamName>, V: AsRef<str>>(name: N, value: V) -> Self {
        Self {
            kind: HttpQueryParamMatchKind::Exact,
            name: name.into(),
            value: value.as_ref().to_string(),
        }
    }

    pub fn matches<N: Into<HttpQueryParamName>, P: AsRef<str>>(name: N, pattern: P) -> Self {
        Self {
            kind: HttpQueryParamMatchKind::RegularExpression,
            name: name.into(),
            value: pattern.as_ref().to_string(),
        }
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpQueryParamMatchKind {
    Exact,
    RegularExpression,
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpQueryParamName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    #[getset(get = "pub")]
    String,
);

impl From<&str> for HttpQueryParamName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
