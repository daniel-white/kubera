use derive_builder::Builder;
use derive_getters::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Builder, Getters, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[builder(setter(into))]
pub struct Ref {
    name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
}

#[derive(Builder, Getters, Debug, Clone)]
pub struct GatewayClassState {
    #[builder(setter(into))]
    name: String,
    #[builder(default)]
    parameter_ref: Option<Ref>,
}

#[derive(Builder, Getters, Debug, Clone)]
pub struct State {}

#[derive(Debug, Clone)]
pub enum StateEvents {
    GatewayClassRegistered(GatewayClassState),
    GatewayClassUnregistered(),
    GatewayParametersRegistered(GatewayClassState),
    GatewayParametersUnregistered(),
}
