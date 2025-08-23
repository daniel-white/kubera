use getset::Getters;
use http::header::IntoHeaderName;
use http::{HeaderMap, HeaderName, HeaderValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::collections::HashSet;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default,
)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub struct HeaderModifierFilter {
    /// Headers to set - will replace existing headers or add new ones
    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::header_map",
        default,
        //skip_serializing_if = "HeaderMap::is_empty"
    )]
    #[schemars(schema_with = "crate::schemars::http_header_map")]
    set: HeaderMap,

    /// Headers to add - will append to existing headers
    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::header_map",
        default,
        skip_serializing_if = "HeaderMap::is_empty"
    )]
    #[schemars(schema_with = "crate::schemars::http_header_map")]
    add: HeaderMap,

    /// Header names to remove
    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::header_name::hash_set",
        default,
        skip_serializing_if = "HashSet::is_empty"
    )]
    #[schemars(schema_with = "crate::schemars::http_header_name_set")]
    remove: HashSet<HeaderName>,
}

impl HeaderModifierFilter {
    pub fn builder() -> HeaderModifierFilterBuilder {
        HeaderModifierFilterBuilder {
            set: HeaderMap::new(),
            add: HeaderMap::new(),
            remove: HashSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct HeaderModifierFilterBuilder {
    set: HeaderMap,
    add: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HeaderModifierFilterBuilder {
    pub fn set_header<N: IntoHeaderName, V: Into<HeaderValue>>(
        &mut self,
        name: N,
        value: V,
    ) -> &mut Self {
        self.set.insert(name, value.into());
        self
    }

    pub fn add_header<N: IntoHeaderName, V: Into<HeaderValue>>(
        &mut self,
        name: N,
        value: V,
    ) -> &mut Self {
        self.add.append(name, value.into());
        self
    }

    /// Remove a header
    pub fn remove_header<H: Into<HeaderName>>(&mut self, header: H) -> &mut Self {
        self.remove.insert(header.into());
        self
    }

    pub fn build(self) -> HeaderModifierFilter {
        HeaderModifierFilter {
            set: self.set,
            add: self.add,
            remove: self.remove,
        }
    }
}
