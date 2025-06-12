mod matches;

use super::super::net::Port;
use super::super::objects::ObjectRef;
use derive_builder::Builder;
use getset::Getters;
pub use matches::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRouteBackendRef {
    #[getset(get = "pub")]
    #[serde(flatten)]
    ref_: ObjectRef,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRouteRuleConfig {
    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    matches: Vec<HttpRouteMatches>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backend_refs: Vec<HttpRouteBackendRef>,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    #[serde(flatten)]
    ref_: ObjectRef,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    hosts: Vec<HostHeaderMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    rules: Vec<HttpRouteRuleConfig>,
}
