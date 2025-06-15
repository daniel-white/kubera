pub mod http;
pub mod net;

use crate::config::gateway::types::http::router::{HttpRoute, HttpRouteBuilder};
use crate::config::gateway::types::net::HostMatch;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::export::regex::Regex;
use serde_valid::{
    error::{MaxLengthError, MinLengthError, PatternError}, Validate, ValidateMaxLength, ValidateMinLength,
    ValidatePattern,
};
use std::fmt::{Display, Formatter};
use strum::EnumString;
use unicase::UniCase;

#[derive(Validate, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveString(UniCase<String>);

impl CaseInsensitiveString {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(UniCase::from(s.as_ref()))
    }
    
    pub fn ends_with(&self, suffix: &Self) -> bool {
        self.0.ends_with(&suffix.0.as_str())
    }
}

impl Display for CaseInsensitiveString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for CaseInsensitiveString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CaseInsensitiveString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl ValidateMinLength for CaseInsensitiveString {
    fn validate_min_length(&self, min: usize) -> Result<(), MinLengthError> {
        self.0.validate_min_length(min)
    }
}

impl ValidateMaxLength for CaseInsensitiveString {
    fn validate_max_length(&self, max: usize) -> Result<(), MaxLengthError> {
        self.0.validate_max_length(max)
    }
}

impl ValidatePattern for CaseInsensitiveString {
    fn validate_pattern(&self, pattern: &Regex) -> Result<(), PatternError> {
        self.0.validate_pattern(pattern)
    }
}

#[derive(
    Validate, Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, EnumString,
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
    #[validate(max_items = 64)]
    hosts: Vec<HostMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,
}

#[derive(Debug, Default)]
pub struct GatewayConfigurationBuilder {
    version: GatewayConfigurationVersion,
    hosts: Vec<HostMatch>,
    http_route_builders: Vec<HttpRouteBuilder>,
}

impl GatewayConfigurationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> GatewayConfiguration {
        GatewayConfiguration {
            version: self.version,
            hosts: self.hosts,
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

    pub fn with_exact_host<S: AsRef<str>>(&mut self, host: S) -> &mut Self {
        self.hosts.push(HostMatch::exactly(host));
        self
    }

    pub fn with_host_suffix<S: AsRef<str>>(&mut self, suffix: S) -> &mut Self {
        self.hosts.push(HostMatch::with_suffix(suffix));
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
