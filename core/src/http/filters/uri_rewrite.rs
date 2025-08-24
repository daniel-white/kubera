use getset::Getters;
use http::uri::Authority;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpUpstreamUriRewrite {
    /// Hostname rewrite
    #[serde(
        with = "http_serde_ext::authority::option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option, into))]
    #[schemars(schema_with = "crate::schemars::authority")]
    authority: Option<Authority>,

    /// Path rewrite
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option))]
    path: Option<HttpUpstreamPathRewrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HttpUpstreamPathRewrite {
    Full(String),
    PrefixMatch(String),
}
