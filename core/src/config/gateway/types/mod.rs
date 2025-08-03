pub mod http;
pub mod net;

use crate::config::gateway::types::http::router::{
    HttpRoute, HttpRouteBuilder, HttpRouteBuilderError,
};
use crate::config::gateway::types::net::{
    ClientAddrs, ClientAddrsBuilder, ErrorResponses, Listener, ListenerBuilder,
    ListenerBuilderError,
};
use crate::net::Port;
use getset::{CloneGetters, CopyGetters, Getters};
use ::http::Error;
use itertools::{Either, Itertools};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::{IpAddr, SocketAddr};
use strum::EnumString;
use thiserror::Error;

#[derive(
    Validate,
    Default,
    Debug,
    Clone,
    Copy,
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
    Validate,
    Getters,
    CloneGetters,
    CopyGetters,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct GatewayConfiguration {
    #[getset(get_copy = "pub")]
    version: GatewayConfigurationVersion,

    #[getset(get = "pub")]
    ipc: Option<IpcConfiguration>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    listeners: Vec<Listener>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,

    #[getset(get = "pub")]
    client_addrs: Option<ClientAddrs>,

    #[getset(get = "pub")]
    error_responses: Option<ErrorResponses>,
}

#[derive(Debug, Default)]
pub struct GatewayConfigurationBuilder {
    version: GatewayConfigurationVersion,
    ipc: Option<IpcConfigurationBuilder>,
    listeners_builders: Vec<ListenerBuilder>,
    http_route_builders: Vec<HttpRouteBuilder>,
    client_addrs_builder: Option<ClientAddrsBuilder>,
    error_responses: Option<ErrorResponses>,
}

#[derive(Debug, Error)]
pub enum GatewayConfigurationBuilderError {
    #[error("Invalid HTTP route at index {0}: {1}")]
    InvalidHttpRoute(usize, HttpRouteBuilderError),
    #[error("Invalid listener at index {0}: {1}")]
    InvalidListener(usize, ListenerBuilderError),
}

impl GatewayConfigurationBuilder {
    pub fn build(self) -> Result<GatewayConfiguration, GatewayConfigurationBuilderError> {
        let (http_routes, errs): (Vec<_>, Vec<_>) = self
            .http_route_builders
            .into_iter()
            .enumerate()
            .map(|(i, b)| (i, b.build()))
            .partition_map(|(i, r)| match r {
                Ok(route) => Either::Left(route),
                Err(err) => {
                    Either::Right(GatewayConfigurationBuilderError::InvalidHttpRoute(i, err))
                }
            });

        if let Some(err) = errs.into_iter().next() {
            return Err(err);
        }

        let (listeners, errs): (Vec<_>, Vec<_>) = self
            .listeners_builders
            .into_iter()
            .enumerate()
            .map(|(i, l)| (i, l.build()))
            .partition_map(|(i, r)| match r {
                Ok(listener) => Either::Left(listener),
                Err(err) => {
                    Either::Right(GatewayConfigurationBuilderError::InvalidListener(i, err))
                }
            });

        if let Some(err) = errs.into_iter().next() {
            return Err(err);
        }

        Ok(GatewayConfiguration {
            version: self.version,
            ipc: self.ipc.map(IpcConfigurationBuilder::build),
            listeners,
            http_routes,
            client_addrs: self.client_addrs_builder.map(ClientAddrsBuilder::build),
            error_responses: self.error_responses,
        })
    }

    pub fn with_version(&mut self, version: GatewayConfigurationVersion) -> &mut Self {
        self.version = version;
        self
    }

    pub fn with_ipc<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut IpcConfigurationBuilder),
    {
        let mut builder = IpcConfigurationBuilder::new();
        factory(&mut builder);
        self.ipc = Some(builder);
        self
    }

    pub fn add_listener<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut ListenerBuilder),
    {
        let mut listener = Listener::builder();
        factory(&mut listener);
        self.listeners_builders.push(listener);
        self
    }

    pub fn add_http_route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut route_builder = HttpRouteBuilder::default();
        factory(&mut route_builder);
        self.http_route_builders.push(route_builder);
        self
    }

    pub fn with_client_addrs<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut ClientAddrsBuilder),
    {
        let mut builder = ClientAddrsBuilder::new();
        factory(&mut builder);
        self.client_addrs_builder = Some(builder);
        self
    }

    pub fn with_error_responses(&mut self, error_responses: ErrorResponses) -> &mut Self {
        self.error_responses = Some(error_responses);
        self
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IpcConfiguration {
    #[getset(get = "pub")]
    endpoint: Option<SocketAddr>,
}

#[derive(Debug, Default)]
pub struct IpcConfigurationBuilder {
    endpoint: Option<SocketAddr>,
}

impl IpcConfigurationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_endpoint(&mut self, ip_addr: IpAddr, port: Port) -> &mut Self {
        let endpoint = SocketAddr::new(ip_addr, port.into());
        self.endpoint = Some(endpoint);
        self
    }

    pub fn build(self) -> IpcConfiguration {
        IpcConfiguration {
            endpoint: self.endpoint,
        }
    }
}
