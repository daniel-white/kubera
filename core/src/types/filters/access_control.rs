use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct Key(String);

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Self(value)
    }
}
