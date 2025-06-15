use crate::config::gateway::types::CaseInsensitiveString;
use getset::Getters;
use schemars::json_schema;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::borrow::Cow;
use std::net::IpAddr;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Port(
    #[validate(minimum = 1)]
    #[validate(maximum = 65535)]
    #[getset(get = "pub")]
    u16,
);

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hostname(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    CaseInsensitiveString,
);

impl Hostname {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(CaseInsensitiveString::new(s))
    }

    pub fn ends_with(&self, suffix: &Hostname) -> bool {
        self.0.ends_with(&suffix.0)
    }
}

impl Into<CaseInsensitiveString> for Hostname {
    fn into(self) -> CaseInsensitiveString {
        self.0
    }
}

impl Into<String> for Hostname {
    fn into(self) -> String {
        self.0.to_string()
    }
}

impl From<&str> for Hostname {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl JsonSchema for Hostname {
    fn schema_name() -> Cow<'static, str> {
        Cow::from(stringify!(Hostname))
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "hostname",
            "minLength": 1,
            "maxLength": 253,
            "pattern": "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$"
        })
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Backend {
    #[getset(get = "pub")]
    weight: i32,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,

    #[getset(get = "pub")]
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Default)]
pub struct BackendBuilder {
    weight: i32,
    port: Option<Port>,
    endpoint_builders: Vec<EndpointBuilder>,
}

impl BackendBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = Some(Port(port));
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
            endpoints: self
                .endpoint_builders
                .into_iter()
                .map(EndpointBuilder::build)
                .collect(),
        }
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct HostMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HostMatchType::is_default"
    )]
    match_type: HostMatchType,

    #[getset(get = "pub")]
    value: Hostname,
}

impl HostMatch {
    pub fn exactly<S: AsRef<str>>(value: S) -> Self {
        Self {
            match_type: HostMatchType::Exact,
            value: Hostname::new(value),
        }
    }

    pub fn with_suffix<S: AsRef<str>>(value: S) -> Self {
        Self {
            match_type: HostMatchType::Suffix,
            value: Hostname::new(value),
        }
    }
}

#[derive(
    Validate, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum HostMatchType {
    #[default]
    Exact,
    Suffix,
}

impl HostMatchType {
    pub fn is_default(&self) -> bool {
        *self == Self::Exact
    }
}
