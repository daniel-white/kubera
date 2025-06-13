pub mod http;
pub mod net;

use crate::config::gateway::types::http::router::{HttpRoute};
use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use strum::EnumString;
use crate::config::gateway::types::net::HostMatch;

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
    hosts: Vec<HostMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,
}
