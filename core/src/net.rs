use std::borrow::Cow;
use getset::Getters;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use crate::CaseInsensitiveString;

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Port(
    #[validate(minimum = 1)]
    #[validate(maximum = 65535)]
    #[getset(get = "pub")]
    u16,
);

impl Port {
    pub fn new(port: u16) -> Self {
        Self(port)
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hostname(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    CaseInsensitiveString,
);

impl Hostname {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(CaseInsensitiveString::new(s))
    }

    pub fn ends_with(&self, suffix: &Hostname) -> bool {
        self.0.ends_with(&suffix.0)
    }
}

impl Into<CaseInsensitiveString> for Hostname {
    fn into(self) -> CaseInsensitiveString {
        self.0
    }
}

impl Into<String> for Hostname {
    fn into(self) -> String {
        self.0.to_string()
    }
}

impl From<&str> for Hostname {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for Hostname {
    fn as_ref(&self) -> &str {
        self.0.0.as_ref()
    }
}

impl JsonSchema for Hostname {
    fn schema_name() -> Cow<'static, str> {
        Cow::from(stringify!(Hostname))
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "hostname",
            "minLength": 1,
            "maxLength": 253,
            "pattern": "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$"
        })
    }
}