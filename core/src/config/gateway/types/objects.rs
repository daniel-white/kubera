use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ObjectName(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[getset(get = "pub")]
    String,
);

#[derive(
    Default,
    Validate,
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
pub struct ObjectNamespace(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")]
    #[getset(get = "pub")]
    String,
);

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ObjectRef {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    namespace: Option<ObjectNamespace>,

    #[getset(get = "pub")]
    name: ObjectName,
}

