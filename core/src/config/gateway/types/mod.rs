pub mod http;
pub mod net;

use crate::config::gateway::types::http::router::{HttpRoute, HttpRouteBuilder};
use crate::config::gateway::types::net::{Listener, ListenerBuilder};
use crate::net::Port;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::{IpAddr, SocketAddr};
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

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GatewayConfiguration {
    #[getset(get = "pub")]
    version: GatewayConfigurationVersion,

    #[getset(get = "pub")]
    controlplane: Option<ControlplaneConfiguration>,

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
    controlplane_configuration_builder: Option<ControlplaneConfigurationBuilder>,
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
            controlplane: self.controlplane_configuration_builder.map(|b| b.build()),
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

    pub fn with_controlplane<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut ControlplaneConfigurationBuilder),
    {
        let mut builder = ControlplaneConfigurationBuilder::new();
        factory(&mut builder);
        self.controlplane_configuration_builder = Some(builder);
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

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ControlplaneConfiguration {
    #[getset(get = "pub")]
    primary_endpoint: Option<SocketAddr>,
}

#[derive(Debug, Default)]
pub struct ControlplaneConfigurationBuilder {
    primary_endpoint: Option<SocketAddr>,
}

impl ControlplaneConfigurationBuilder {
    pub fn new() -> Self {
        Self {
            primary_endpoint: None,
        }
    }

    pub fn with_primary_endpoint(&mut self, ip_addr: &IpAddr, port: &Port) -> &mut Self {
        let socket_addr = SocketAddr::new(ip_addr.clone(), (*port).into());
        self.primary_endpoint = Some(socket_addr);
        self
    }

    pub fn build(self) -> ControlplaneConfiguration {
        ControlplaneConfiguration {
            primary_endpoint: self.primary_endpoint,
        }
    }
}
