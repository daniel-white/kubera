use crate::config::gateway::types::net::Hostname;
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
    pub fn exactly<S: AsRef<str>>(hostname: S) -> Self {
        Self {
            match_type: HostHeaderMatchType::Exact,
            value: Hostname::new(hostname),
        }
    }

    pub fn with_suffix<S: AsRef<str>>(suffix: S) -> Self {
        Self {
            match_type: HostHeaderMatchType::Suffix,
            value: Hostname::new(suffix),
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
