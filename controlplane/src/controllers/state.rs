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
