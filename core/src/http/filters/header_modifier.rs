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
#[serde(rename_all = "camelCase")]
pub struct HttpHeaderModifierFilter {
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

impl HttpHeaderModifierFilter {
    pub fn builder() -> HttpHeaderModifierFilterBuilder {
        HttpHeaderModifierFilterBuilder {
            set: HeaderMap::new(),
            add: HeaderMap::new(),
            remove: HashSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpHeaderModifierFilterBuilder {
    set: HeaderMap,
    add: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HttpHeaderModifierFilterBuilder {
    pub fn set_header<H: IntoHeaderName, V: Into<HeaderValue>>(
        &mut self,
        header: H,
        value: V,
    ) -> &mut Self {
        self.set.insert(header, value.into());
        self
    }

    pub fn add_header<H: IntoHeaderName, V: Into<HeaderValue>>(
        &mut self,
        header: H,
        value: V,
    ) -> &mut Self {
        self.add.append(header, value.into());
        self
    }

    /// Remove a header
    pub fn remove_header<H: Into<HeaderName>>(&mut self, header: H) -> &mut Self {
        self.remove.insert(header.into());
        self
    }

    pub fn build(self) -> HttpHeaderModifierFilter {
        HttpHeaderModifierFilter {
            set: self.set,
            add: self.add,
            remove: self.remove,
        }
    }
}
