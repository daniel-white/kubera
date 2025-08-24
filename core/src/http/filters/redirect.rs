use getset::Getters;
use http::uri::{Authority, Scheme};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRedirectFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: HttpRedirectKind,

    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::scheme::option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[builder(default, setter(strip_option, into))]
    #[schemars(schema_with = "crate::schemars::scheme")]
    scheme: Option<Scheme>,

    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::authority::option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[builder(default, setter(strip_option, into))]
    #[schemars(schema_with = "crate::schemars::scheme")]
    authority: Option<Authority>,

    /// Redirect path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option, into))]
    path: Option<HttpRedirectPathRewrite>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpRedirectKind {
    Permanent,
    #[default]
    Temporary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum HttpRedirectPathRewrite {
    Full(String),
    PrefixMatch(String),
}
