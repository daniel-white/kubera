use crate::schemars::cidr_array;
use getset::Getters;
use ipnet::IpNet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct AccessControlFilterKey(String);

impl<S: AsRef<str>> From<S> for AccessControlFilterKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder, Getters,
)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub struct AccessControlFilterRef {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: AccessControlFilterKey,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub struct AccessControlFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: AccessControlFilterKey,

    #[getset(get = "pub")]
    effect: AccessControlEffect,

    #[getset(get = "pub")]
    clients: AccessControlClients,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub enum AccessControlEffect {
    Allow,
    Deny,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub struct AccessControlClients {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ips: Vec<IpAddr>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "cidr_array")]
    ip_ranges: Vec<IpNet>,
}
