use crate::net::{Hostname, Port};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::IpAddr;
use thiserror::Error;
use typed_builder::TypedBuilder;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Backend {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weight: Option<i32>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,

    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Error)]
pub enum BackendBuilderError {
    #[error("Backend name is required")]
    MissingName,
}

impl BackendBuilder {
    pub fn named<S: AsRef<str>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.as_ref().to_string());
        self
    }

    pub fn with_namespace<S: AsRef<str>>(&mut self, namespace: Option<S>) -> &mut Self {
        self.namespace = namespace.map(|n| n.as_ref().to_string());
        self
    }

    pub fn with_port(&mut self, port: Option<Port>) -> &mut Self {
        self.port = port;
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

    pub fn build(self) -> Result<Backend, BackendBuilderError> {
        let name = self.name.ok_or(BackendBuilderError::MissingName)?;
        Ok(Backend {
            weight: self.weight,
            port: self.port,
            name,
            namespace: self.namespace,
            endpoints: self
                .endpoint_builders
                .into_iter()
                .map(EndpointBuilder::build)
                .collect(),
        })
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
    pub fn builder() -> ListenerBuilder {
        ListenerBuilder::new()
    }
}

#[derive(Debug)]
pub struct ListenerBuilder {
    name: Option<String>,
    host: Option<HostnameMatch>,
    port: Option<Port>,
    protocol: Option<String>,
}

#[derive(Debug, Error)]
pub enum ListenerBuilderError {
    #[error("Listener name is required")]
    MissingName,
    #[error("Listener port is required")]
    MissingPort,
    #[error("Listener protocol is required")]
    MissingProtocol,
}

impl ListenerBuilder {
    fn new() -> Self {
        Self {
            name: None,
            host: None,
            port: None,
            protocol: None,
        }
    }

    pub fn build(self) -> Result<Listener, ListenerBuilderError> {
        Ok(Listener {
            name: self.name.ok_or(ListenerBuilderError::MissingName)?,
            host: self.host,
            port: self.port.ok_or(ListenerBuilderError::MissingPort)?,
            protocol: self.protocol.ok_or(ListenerBuilderError::MissingProtocol)?,
        })
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

    pub fn with_port(&mut self, port: Port) -> &mut Self {
        self.port = Some(port);
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

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum ErrorResponseKind {
    Empty,
    #[default]
    Html,
    ProblemDetail,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, Default, TypedBuilder,
)]
pub struct ErrorResponses {
    #[getset(get = "pub")]
    kind: ErrorResponseKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option))]
    problem_detail: Option<ProblemDetailErrorResponse>,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, Default, TypedBuilder,
)]
pub struct ProblemDetailErrorResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(setter(into))]
    authority: Option<String>,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, Default, TypedBuilder,
)]
pub struct StaticResponses {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    responses: Vec<StaticResponse>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder)]
pub struct StaticResponse {
    #[getset(get = "pub")]
    key: String,

    #[getset(get = "pub")]
    status_code: u16,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    version_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option))]
    body: Option<StaticResponseBody>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder)]
pub struct StaticResponseBody {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    content_type: String,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    identifier: String,
}
