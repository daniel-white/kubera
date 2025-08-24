use crate::schemars::cidr_array;
use getset::Getters;
use ipnet::IpNet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpAccessControlFilterKey(String);

impl<S: AsRef<str>> From<S> for HttpAccessControlFilterKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder, Getters,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpAccessControlFilterRef {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpAccessControlFilterKey,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder, Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpAccessControlFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpAccessControlFilterKey,

    #[getset(get = "pub")]
    effect: HttpAccessControlEffect,

    #[getset(get = "pub")]
    clients: HttpAccessControlClients,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HttpAccessControlEffect {
    Allow,
    Deny,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder, Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpAccessControlClients {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    ips: Vec<IpAddr>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "cidr_array")]
    #[builder(default)]
    ip_ranges: Vec<IpNet>,
}
