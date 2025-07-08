use crate::net::{Hostname, Port};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::IpAddr;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash,
)]
pub struct Backend {
    #[getset(get = "pub")]
    weight: Option<i32>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,

    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    namespace: Option<String>,

    #[getset(get = "pub")]
    endpoints: Vec<Endpoint>,
}

#[derive(Default, Debug)]
pub struct BackendBuilder {
    weight: Option<i32>,
    port: Option<Port>,
    name: Option<String>,
    namespace: Option<String>,
    endpoint_builders: Vec<EndpointBuilder>,
}

impl BackendBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn named<S: AsRef<str>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.as_ref().to_string());
        self
    }

    pub fn with_namespace<S: AsRef<str>>(&mut self, namespace: Option<S>) -> &mut Self {
        self.namespace = namespace.map(|n| n.as_ref().to_string());
        self
    }

    pub fn with_port(&mut self, port: Option<u16>) -> &mut Self {
        self.port = port.map(Port::new);
        self
    }

    pub fn with_weight(&mut self, weight: Option<i32>) -> &mut Self {
        self.weight = weight;
        self
    }

    pub fn add_endpoint<F>(&mut self, address: IpAddr, factory: F) -> &mut Self
    where
        F: FnOnce(&mut EndpointBuilder),
    {
        let mut endpoint_builder = EndpointBuilder::new(address);
        factory(&mut endpoint_builder);
        self.endpoint_builders.push(endpoint_builder);
        self
    }

    pub fn build(self) -> Backend {
        Backend {
            weight: self.weight,
            port: self.port,
            name: self.name.expect("Backend name is required"),
            namespace: self.namespace,
            endpoints: self
                .endpoint_builders
                .into_iter()
                .map(EndpointBuilder::build)
                .collect(),
        }
    }
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema,
)]
pub struct Endpoint {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<String>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zone: Option<String>,

    #[getset(get = "pub")]
    address: IpAddr,
}

#[derive(Debug)]
pub struct EndpointBuilder {
    node: Option<String>,
    zone: Option<String>,
    address: IpAddr,
}

impl EndpointBuilder {
    fn new(address: IpAddr) -> Self {
        Self {
            node: None,
            zone: None,
            address,
        }
    }

    pub fn build(self) -> Endpoint {
        Endpoint {
            node: self.node,
            zone: self.zone,
            address: self.address,
        }
    }

    pub fn with_node<S: AsRef<str>>(&mut self, node: S) -> &mut Self {
        self.node = Some(node.as_ref().to_string());
        self
    }

    pub fn with_zone<S: AsRef<str>>(&mut self, zone: S) -> &mut Self {
        self.zone = Some(zone.as_ref().to_string());
        self
    }
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct Listener {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<HostnameMatch>,

    #[getset(get = "pub")]
    port: Port,

    #[getset(get = "pub")]
    protocol: String,
}

impl Listener {
    pub fn new_builder() -> ListenerBuilder {
        ListenerBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct ListenerBuilder {
    name: Option<String>,
    host: Option<HostnameMatch>,
    port: Option<Port>,
    protocol: Option<String>,
}

impl ListenerBuilder {
    pub fn build(self) -> Listener {
        Listener {
            name: self.name.expect("Listener name is required"),
            host: self.host,
            port: self.port.expect("Listener port is required"),
            protocol: self.protocol.expect("Listener protocol is required"),
        }
    }

    pub fn with_name<S: AsRef<str>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.as_ref().to_string());
        self
    }

    pub fn with_exact_hostname<S: AsRef<str>>(&mut self, hostname: S) -> &mut Self {
        self.host = Some(HostnameMatch::exactly(hostname));
        self
    }

    pub fn with_hostname_suffix<S: AsRef<str>>(&mut self, suffix: S) -> &mut Self {
        self.host = Some(HostnameMatch::with_suffix(suffix));
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = Some(Port::new(port));
        self
    }

    pub fn with_protocol<S: AsRef<str>>(&mut self, protocol: S) -> &mut Self {
        self.protocol = Some(protocol.as_ref().to_string());
        self
    }
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HostnameMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HostnameMatchType::is_default"
    )]
    match_type: HostnameMatchType,

    #[getset(get = "pub")]
    value: Hostname,
}

impl HostnameMatch {
    pub fn exactly<S: AsRef<str>>(value: S) -> Self {
        Self {
            match_type: HostnameMatchType::Exact,
            value: Hostname::new(value),
        }
    }

    pub fn with_suffix<S: AsRef<str>>(value: S) -> Self {
        Self {
            match_type: HostnameMatchType::Suffix,
            value: Hostname::new(value),
        }
    }
}

#[derive(
    Validate, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum HostnameMatchType {
    #[default]
    Exact,
    Suffix,
}

impl HostnameMatchType {
    pub fn is_default(&self) -> bool {
        *self == Self::Exact
    }
}
