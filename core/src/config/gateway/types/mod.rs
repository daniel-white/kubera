pub mod http;
pub mod net;
pub mod objects;

use crate::config::gateway::types::http::router::{HostHeaderMatch, HttpRoute};
use crate::config::gateway::types::net::Backend;
use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use strum::EnumString;

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, EnumString)]
#[serde(rename_all = "lowercase")]
pub enum GatewayConfigurationVersion {
    V1Alpha1,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct GatewayConfiguration {
    #[getset(get = "pub")]
    version: GatewayConfigurationVersion,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    hosts: Vec<HostHeaderMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    service_backends: Vec<Backend>,
}
