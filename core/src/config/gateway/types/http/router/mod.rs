mod matches;

use crate::config::gateway::types::net::Backend;
use derive_builder::Builder;
use getset::Getters;
pub use matches::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRouteRule {
    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    matches: Vec<HttpRouteRuleMatches>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backends: Vec<Backend>,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    #[serde(rename = "host_headers")]
    host_header_matches: Vec<HostHeaderMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    rules: Vec<HttpRouteRule>,
}
