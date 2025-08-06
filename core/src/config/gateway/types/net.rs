use crate::config::gateway::types::http::filters::RequestHeaderModifier;
use crate::net::{Hostname, Port};
use getset::Getters;
use ipnet::IpNet;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::IpAddr;
use thiserror::Error;
use typed_builder::TypedBuilder;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

    /// Request header modifier for this backend
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_header_modifier: Option<RequestHeaderModifier>,
}

#[derive(Default, Debug)]
pub struct BackendBuilder {
    weight: Option<i32>,
    port: Option<Port>,
    name: Option<String>,
    namespace: Option<String>,
    endpoint_builders: Vec<EndpointBuilder>,
    request_header_modifier: Option<RequestHeaderModifier>,
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

    /// Set request header modifier for this backend
    pub fn with_request_header_modifier(&mut self, modifier: RequestHeaderModifier) -> &mut Self {
        self.request_header_modifier = Some(modifier);
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
            request_header_modifier: self.request_header_modifier,
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
pub enum ClientAddrsSource {
    #[default]
    None,
    Header,
    Proxies,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, Default)]
pub struct ClientAddrs {
    #[getset(get = "pub")]
    source: ClientAddrsSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    proxies: Option<TrustedProxies>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyHeaders {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters)]
#[serde(rename_all = "camelCase")]
pub struct TrustedProxies {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[getset(get = "pub")]
    trusted_ips: Vec<IpAddr>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "cidr_array_schema")]
    #[getset(get = "pub")]
    trusted_ranges: Vec<IpNet>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[getset(get = "pub")]
    trusted_headers: Vec<ProxyHeaders>,
}

#[derive(Debug, Default)]
pub struct TrustedProxiesBuilder {
    trusted_ips: Vec<IpAddr>,
    trusted_ranges: Vec<IpNet>,
    trusted_headers: Vec<ProxyHeaders>,
}

impl TrustedProxiesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trust_local_ranges(&mut self) -> &mut Self {
        #[allow(clippy::unwrap_used)] // These are hardcoded and should not fail
        let mut ranges: Vec<_> = vec![
            // IPV4 Loopback
            "127.0.0.0/8".parse().unwrap(),
            // IPV4 Private Networks
            "10.0.0.0/8".parse().unwrap(),
            "172.16.0.0/12".parse().unwrap(),
            "192.168.0.0/16".parse().unwrap(),
            // IPV6 Loopback
            "::1/128".parse().unwrap(),
            // IPV6 Private network
            "fd00::/8".parse().unwrap(),
        ];
        self.trusted_ranges.append(&mut ranges);
        self
    }

    pub fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.trusted_ips.push(ip);
        self
    }

    pub fn add_trusted_range(&mut self, cidr: IpNet) -> &mut Self {
        self.trusted_ranges.push(cidr);
        self
    }

    pub fn add_trusted_header(&mut self, header: ProxyHeaders) -> &mut Self {
        self.trusted_headers.push(header);
        self
    }

    pub fn build(self) -> TrustedProxies {
        TrustedProxies {
            trusted_ips: self.trusted_ips,
            trusted_ranges: self.trusted_ranges,
            trusted_headers: self.trusted_headers,
        }
    }
}

pub fn cidr_array_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": {
            "type": "string",
            "format": "cidr"
        },
        "minItems": 1,
        "uniqueItems": true
    })
}

#[derive(Debug, Default)]
pub struct ClientAddrsBuilder {
    source: ClientAddrsSource,
    header: Option<String>,
    proxies: Option<TrustedProxiesBuilder>,
}

impl ClientAddrsBuilder {
    pub fn new() -> Self {
        Self {
            source: ClientAddrsSource::None,
            header: None,
            proxies: None,
        }
    }

    pub fn trust_header<S: AsRef<str>>(&mut self, header: S) -> &mut Self {
        self.source = ClientAddrsSource::Header;
        self.header = Some(header.as_ref().to_string());
        self.proxies = None;
        self
    }

    pub fn trust_proxies<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut TrustedProxiesBuilder),
    {
        self.source = ClientAddrsSource::Proxies;
        let mut builder = TrustedProxiesBuilder::new();
        factory(&mut builder);
        self.proxies = Some(builder);
        self
    }

    pub fn build(self) -> ClientAddrs {
        match self.source {
            ClientAddrsSource::None => ClientAddrs::default(),
            ClientAddrsSource::Header => ClientAddrs {
                source: ClientAddrsSource::Header,
                header: self.header,
                ..Default::default()
            },
            ClientAddrsSource::Proxies => ClientAddrs {
                source: ClientAddrsSource::Proxies,
                proxies: self.proxies.map(TrustedProxiesBuilder::build),
                ..Default::default()
            },
        }
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
