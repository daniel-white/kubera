use getset::Getters;
use http::HeaderName;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::str::FromStr;
use thiserror::Error;
use typed_builder::TypedBuilder;

/// HTTP Route Filter - matches Gateway API structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct HttpRouteFilter {
    /// Filter type - matches Gateway API filter types
    #[serde(rename = "type")]
    pub filter_type: HttpRouteFilterType,

    /// `RequestHeaderModifier` defines a schema for a filter that modifies request headers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_modifier: Option<RequestHeaderModifier>,

    /// `ResponseHeaderModifier` defines a schema for a filter that modifies response headers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_header_modifier: Option<ResponseHeaderModifier>,

    /// `RequestMirror` defines a schema for a filter that mirrors requests
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_mirror: Option<RequestMirror>,

    /// `RequestRedirect` defines a schema for a filter that responds with HTTP redirection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_redirect: Option<RequestRedirect>,

    /// `URLRewrite` defines a schema for a filter that modifies URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_rewrite: Option<URLRewrite>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext_static_response: Option<ExtStaticResponseRef>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext_access_control: Option<ExtAccessControlRef>,
}

/// HTTP Route Filter Types - matches Gateway API filter types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpRouteFilterType {
    #[serde(rename = "RequestHeaderModifier")]
    RequestHeaderModifier,
    #[serde(rename = "ResponseHeaderModifier")]
    ResponseHeaderModifier,
    #[serde(rename = "RequestMirror")]
    RequestMirror,
    #[serde(rename = "RequestRedirect")]
    RequestRedirect,
    #[serde(rename = "URLRewrite")]
    URLRewrite,
    #[serde(rename = "StaticResponse")]
    ExtStaticResponse,
    #[serde(rename = "AccessControl")]
    ExtAccessControl,
}

/// Request header modification filter - matches Gateway API `RequestHeaderModifier` structure
#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default,
)]
pub struct RequestHeaderModifier {
    /// Headers to set - will replace existing headers or add new ones
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<Vec<HTTPHeader>>,

    /// Headers to add - will append to existing headers
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<HTTPHeader>>,

    /// Header names to remove
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    pub remove: Option<Vec<String>>,
}

/// Response header modification filter - matches Gateway API `ResponseHeaderModifier` structure
#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default,
)]
pub struct ResponseHeaderModifier {
    /// Headers to set - will replace existing headers or add new ones
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<Vec<HTTPHeader>>,

    /// Headers to add - will append to existing headers
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<HTTPHeader>>,

    /// Header names to remove
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(max_items = 16)]
    pub remove: Option<Vec<String>>,
}

/// HTTP Header name-value pair - matches Gateway API structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HTTPHeader {
    /// Header name
    pub name: String,
    /// Header value
    pub value: String,
}

/// Request Mirror filter - placeholder for future implementation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestMirror {
    /// Backend to mirror requests to
    #[serde(rename = "backendRef")]
    pub backend_ref: BackendRef,
}

/// Request Redirect filter - placeholder for future implementation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct RequestRedirect {
    /// Redirect scheme
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// Redirect hostname
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// Redirect path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathRewrite>,
    /// Redirect port
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Status code for redirect
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

/// URL Rewrite filter - placeholder for future implementation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct URLRewrite {
    /// Hostname rewrite
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// Path rewrite
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathRewrite>,
}

/// Path rewrite configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PathRewrite {
    /// Type of path rewrite
    #[serde(rename = "type")]
    pub rewrite_type: PathRewriteType,
    /// Replacement value for full path replacement
    pub replace_full_path: Option<String>,
    /// Prefix replacement for prefix match replacement
    pub replace_prefix_match: Option<String>,
}

/// Path rewrite types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PathRewriteType {
    #[serde(rename = "ReplaceFullPath")]
    ReplaceFullPath,
    #[serde(rename = "ReplacePrefixMatch")]
    ReplacePrefixMatch,
}

/// Backend reference for filters
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackendRef {
    /// Backend name
    pub name: String,
    /// Backend namespace
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Backend port
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StaticResponseRef {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtAccessControlRef {
    pub key: String,
}

#[derive(Debug, Error)]
pub enum HTTPRouteFilterBuilderError {
    #[error("Header name cannot be empty")]
    EmptyHeaderName,
    #[error("Header value cannot be empty")]
    EmptyHeaderValue,
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(String),
}

#[derive(Debug, Default)]
pub struct RequestHeaderModifierBuilder {
    set: Vec<HTTPHeader>,
    add: Vec<HTTPHeader>,
    remove: Vec<String>,
}

impl RequestHeaderModifierBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a header (replaces existing)
    pub fn set_header<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: K,
        value: V,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();
        let value = value.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }
        if value.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderValue);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.set.push(HTTPHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        Ok(self)
    }

    /// Add a header (appends to existing)
    pub fn add_header<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: K,
        value: V,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();
        let value = value.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }
        if value.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderValue);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.add.push(HTTPHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        Ok(self)
    }

    /// Remove a header
    pub fn remove_header<K: AsRef<str>>(
        &mut self,
        name: K,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.remove.push(name.to_string());
        Ok(self)
    }

    pub fn build(self) -> RequestHeaderModifier {
        RequestHeaderModifier {
            set: if self.set.is_empty() {
                None
            } else {
                Some(self.set)
            },
            add: if self.add.is_empty() {
                None
            } else {
                Some(self.add)
            },
            remove: if self.remove.is_empty() {
                None
            } else {
                Some(self.remove)
            },
        }
    }
}

impl RequestHeaderModifier {
    pub fn builder() -> RequestHeaderModifierBuilder {
        RequestHeaderModifierBuilder::new()
    }

    /// Check if this modifier has any operations
    pub fn is_empty(&self) -> bool {
        self.set.as_ref().is_none_or(Vec::is_empty)
            && self.add.as_ref().is_none_or(Vec::is_empty)
            && self.remove.as_ref().is_none_or(Vec::is_empty)
    }
}

#[derive(Debug, Default)]
pub struct ResponseHeaderModifierBuilder {
    set: Vec<HTTPHeader>,
    add: Vec<HTTPHeader>,
    remove: Vec<String>,
}

impl ResponseHeaderModifierBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a header (replaces existing)
    pub fn set_header<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: K,
        value: V,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();
        let value = value.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }
        if value.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderValue);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.set.push(HTTPHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        Ok(self)
    }

    /// Add a header (appends to existing)
    pub fn add_header<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: K,
        value: V,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();
        let value = value.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }
        if value.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderValue);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.add.push(HTTPHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
        Ok(self)
    }

    /// Remove a header
    pub fn remove_header<K: AsRef<str>>(
        &mut self,
        name: K,
    ) -> Result<&mut Self, HTTPRouteFilterBuilderError> {
        let name = name.as_ref();

        if name.is_empty() {
            return Err(HTTPRouteFilterBuilderError::EmptyHeaderName);
        }

        // Use http crate for proper header name validation
        HeaderName::from_str(name)
            .map_err(|_| HTTPRouteFilterBuilderError::InvalidHeaderName(name.to_string()))?;

        self.remove.push(name.to_string());
        Ok(self)
    }

    pub fn build(self) -> ResponseHeaderModifier {
        ResponseHeaderModifier {
            set: if self.set.is_empty() {
                None
            } else {
                Some(self.set)
            },
            add: if self.add.is_empty() {
                None
            } else {
                Some(self.add)
            },
            remove: if self.remove.is_empty() {
                None
            } else {
                Some(self.remove)
            },
        }
    }
}

impl ResponseHeaderModifier {
    pub fn builder() -> ResponseHeaderModifierBuilder {
        ResponseHeaderModifierBuilder::new()
    }

    /// Check if this modifier has any operations
    pub fn is_empty(&self) -> bool {
        self.set.as_ref().is_none_or(Vec::is_empty)
            && self.add.as_ref().is_none_or(Vec::is_empty)
            && self.remove.as_ref().is_none_or(Vec::is_empty)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
pub struct ExtStaticResponseRef {
    #[getset(get = "pub")]
    key: String,
}
