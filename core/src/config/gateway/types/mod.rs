pub mod http;
pub mod net;

use crate::config::gateway::types::http::router::{HttpRoute, HttpRouteBuilder};
use crate::config::gateway::types::net::{Listener, ListenerBuilder};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use strum::EnumString;

#[derive(
    Validate,
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    Hash,
    EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum GatewayConfigurationVersion {
    #[default]
    V1Alpha1,
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
pub struct GatewayConfiguration {
    #[getset(get = "pub")]
    version: GatewayConfigurationVersion,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    listeners: Vec<Listener>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,
}

#[derive(Debug, Default)]
pub struct GatewayConfigurationBuilder {
    version: GatewayConfigurationVersion,
    listeners: Vec<Listener>,
    http_route_builders: Vec<HttpRouteBuilder>,
}

impl GatewayConfigurationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> GatewayConfiguration {
        GatewayConfiguration {
            version: self.version,
            listeners: self.listeners,
            http_routes: self
                .http_route_builders
                .into_iter()
                .map(HttpRouteBuilder::build)
                .collect(),
        }
    }

    pub fn with_version(&mut self, version: GatewayConfigurationVersion) -> &mut Self {
        self.version = version;
        self
    }

    pub fn add_listener<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut ListenerBuilder),
    {
        let mut listener = Listener::new_builder();
        factory(&mut listener);
        let listener = listener.build();
        self.listeners.push(listener);
        self
    }

    pub fn add_http_route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut route_builder = HttpRouteBuilder::new();
        factory(&mut route_builder);
        self.http_route_builders.push(route_builder);
        self
    }
}
