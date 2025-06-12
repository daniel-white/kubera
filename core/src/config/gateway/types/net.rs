use crate::config::gateway::types::objects::ObjectRef;
use derive_builder::Builder;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::IpAddr;

#[derive(
    Validate, Default, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct Port(
    #[validate(minimum = 1)]
    #[validate(maximum = 65535)]
    #[getset(get = "pub")]
    u16,
);

#[derive(
    Validate,
    Default,
    Getters,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct Hostname(
    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    #[getset(get = "pub")]
    String,
);

#[derive(
    Validate,
    Default,
    Builder,
    Getters,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct BackendEndpoints {
    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zone: Option<String>,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<String>,

    #[getset(get = "pub")]
    addresses: Vec<IpAddr>,
}

#[derive(
    Validate, Getters, Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct Backend {
    #[getset(get = "pub")]
    #[serde(flatten)]
    ref_: ObjectRef,

    #[getset(get = "pub")]
    endpoints: Vec<BackendEndpoints>,
}
