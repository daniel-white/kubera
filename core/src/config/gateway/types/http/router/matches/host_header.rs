use crate::config::gateway::types::net::Hostname;
use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HostHeaderMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HostHeaderMatchType::is_default"
    )]
    match_type: HostHeaderMatchType,

    #[getset(get = "pub")]
    value: Hostname,
}

impl HostHeaderMatch {
    pub fn with_exact_host(value: &str) -> Self {
        Self {
            match_type: HostHeaderMatchType::Exact,
            value: value.into(),
        }
    }

    pub fn with_host_suffix(value: &str) -> Self {
        Self {
            match_type: HostHeaderMatchType::Exact,
            value: value.into(),
        }
    }
}

#[derive(
    Validate, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum HostHeaderMatchType {
    #[default]
    Exact,
    Suffix,
}

impl HostHeaderMatchType {
    pub fn is_default(&self) -> bool {
        *self == Self::Exact
    }
}
