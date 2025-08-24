use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::SocketAddr;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpBackend {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weight: Option<i32>,

    #[getset(get = "pub")]
    endpoints: Vec<HttpBackendEndpoint>,
}

impl HttpBackend {
    pub fn builder() -> HttpBackendBuilder {
        HttpBackendBuilder {
            weight: None,
            endpoint_builders: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpBackendBuilder {
    weight: Option<i32>,
    endpoint_builders: Vec<HttpBackendEndpointBuilder>,
}

impl HttpBackendBuilder {
    pub fn build(self) -> HttpBackend {
        HttpBackend {
            weight: self.weight,
            endpoints: self
                .endpoint_builders
                .into_iter()
                .map(HttpBackendEndpointBuilder::build)
                .collect(),
        }
    }

    pub fn with_weight(&mut self, weight: i32) -> &mut Self {
        self.weight = Some(weight);
        self
    }

    pub fn add_endpoint<F>(&mut self, addr: SocketAddr, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpBackendEndpointBuilder),
    {
        let mut builder = HttpBackendEndpoint::builder(addr);
        factory(&mut builder);
        self.endpoint_builders.push(builder);
        self
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpBackendEndpoint {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<String>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zone: Option<String>,

    #[getset(get = "pub")]
    addr: SocketAddr,
}

impl HttpBackendEndpoint {
    pub fn builder<A: Into<SocketAddr>>(addr: A) -> HttpBackendEndpointBuilder {
        HttpBackendEndpointBuilder {
            node: None,
            zone: None,
            addr: addr.into(),
        }
    }
}

#[derive(Debug)]
pub struct HttpBackendEndpointBuilder {
    node: Option<String>,
    zone: Option<String>,
    addr: SocketAddr,
}

impl HttpBackendEndpointBuilder {
    pub fn build(self) -> HttpBackendEndpoint {
        HttpBackendEndpoint {
            node: self.node,
            zone: self.zone,
            addr: self.addr,
        }
    }

    pub fn with_node<N: Into<String>>(&mut self, node: N) -> &mut Self {
        self.node = Some(node.into());
        self
    }

    pub fn with_zone<Z: Into<String>>(&mut self, zone: Z) -> &mut Self {
        self.zone = Some(zone.into());
        self
    }
}
