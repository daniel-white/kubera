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
pub struct HttpPathMatch {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: HttpPathMatchKind,

    #[getset(get = "pub")]
    #[validate(max_length = 1024)]
    #[builder(setter(into))]
    value: String,
}

impl HttpPathMatch {
    pub fn exactly<S: AsRef<str>>(path: S) -> Self {
        Self {
            kind: HttpPathMatchKind::Exact,
            value: path.as_ref().to_string(),
        }
    }

    pub fn with_prefix<S: AsRef<str>>(prefix: S) -> Self {
        Self {
            kind: HttpPathMatchKind::Prefix,
            value: prefix.as_ref().to_string(),
        }
    }

    pub fn matching<S: AsRef<str>>(pattern: S) -> Self {
        Self {
            kind: HttpPathMatchKind::RegularExpression,
            value: pattern.as_ref().to_string(),
        }
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpPathMatchKind {
    Exact,
    Prefix,
    RegularExpression,
}
