use crate::config::gateway::types::objects::ObjectRef;
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

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceBackend {
    #[getset(get = "pub")]
    #[serde(flatten)]
    ref_: ObjectRef,

    #[getset(get = "pub")]
    addresses: Vec<IpAddr>,
}
