use crate::http::filters::access_control::AccessControlFilterRef;
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
    pub ext_access_control: Option<AccessControlFilterRef>,
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

#[derive(Debug, Error)]
pub enum HTTPRouteFilterBuilderError {
    #[error("Header name cannot be empty")]
    EmptyHeaderName,
    #[error("Header value cannot be empty")]
    EmptyHeaderValue,
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(String),
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
pub struct ExtStaticResponseRef {
    #[getset(get = "pub")]
    key: String,
}
