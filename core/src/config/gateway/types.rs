use derive_builder::Builder;
use getset::Getters;
use http::{HeaderName, Method};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use strum::EnumString;

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, EnumString)]
#[serde(rename_all = "lowercase")]
pub enum GatewayConfigurationVersion {
    V1Alpha1,
}

#[derive(
    Default, Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct BackendGroupName(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(
        pattern = "^$\\|^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$"
    )]
    #[getset(get = "pub")]
    String,
);

impl BackendGroupName {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackendKindName(
    #[validate(min_length = 1)]
    #[validate(max_length = 63)]
    #[validate(pattern = "^[a-zA-Z]([-a-zA-Z0-9]*[a-zA-Z0-9])?$")]
    #[getset(get = "pub")]
    String,
);

impl BackendKindName {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_default(&self) -> bool {
        self.is_empty() || self.0 == "Service"
    }
}

impl Default for BackendKindName {
    fn default() -> Self {
        BackendKindName("Service".to_string())
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackendObjectName(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[getset(get = "pub")]
    String,
);

#[derive(
    Default, Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct BackendNamespace(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")]
    #[getset(get = "pub")]
    String,
);

impl BackendNamespace {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(
    Validate, Default, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct Port(
    #[validate(minimum = 1)]
    #[validate(maximum = 65535)]
    #[getset(get = "pub")]
    u16,
);

#[derive(Validate, Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BackendPort {
    #[default]
    #[serde(skip)]
    NotSpecified,
    Port(Port),
}

impl BackendPort {
    pub fn is_default(&self) -> bool {
        self == &BackendPort::NotSpecified
    }
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct GatewayConfiguration {
    #[getset(get = "pub")]
    version: GatewayConfigurationVersion,
    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    hosts: Vec<Host>,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    hostnames: Vec<HostnameMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    http_routes: Vec<HttpRoute>,
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hostname(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    #[getset(get = "pub")]
    String,
);

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HostnameMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HostnameMatchType::is_default"
    )]
    match_type: HostnameMatchType,

    #[getset(get = "pub")]
    value: Hostname,
}

#[derive(Validate, Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HostnameMatchType {
    #[default]
    Exact,
    Suffix,
}

impl HostnameMatchType {
    pub fn is_default(&self) -> bool {
        *self == HostnameMatchType::Exact
    }
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    name: HttpRouteName,

    #[getset(get = "pub")]
    #[validate(max_items = 64)]
    matches: Vec<HttpRouteMatch>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backends: Vec<HttpBackendRef>,
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpRouteName(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    String,
);

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteMatch {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    paths: Vec<HttpPathMatch>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    headers: Vec<HttpHeaderMatch>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    query_params: Vec<HttpQueryParamMatch>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    methods: Vec<HttpMethodMatch>,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpBackendRef {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "BackendGroupName::is_empty")]
    group: BackendGroupName,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "BackendKindName::is_default")]
    kind: BackendKindName,

    #[getset(get = "pub")]
    name: BackendObjectName,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "BackendNamespace::is_empty")]
    namespace: BackendNamespace,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "BackendPort::is_default")]
    port: BackendPort,
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpPathMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpPathMatchType::is_default"
    )]
    match_type: HttpPathMatchType,

    #[getset(get = "pub")]
    #[validate(max_length = 1024)]
    value: String,
}

impl Default for HttpPathMatch {
    fn default() -> Self {
        HttpPathMatch {
            match_type: HttpPathMatchType::Prefix,
            value: "/".to_string(),
        }
    }
}

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpPathMatchType {
    Exact,
    #[default]
    Prefix,
    RegularExpression,
}

impl HttpPathMatchType {
    fn is_default(&self) -> bool {
        *self == HttpPathMatchType::Prefix
    }
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpHeaderMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpHeaderMatchType::is_default"
    )]
    match_type: HttpHeaderMatchType,

    #[getset(get = "pub")]
    name: HttpHeaderName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 4096)]
    value: String,
}

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpHeaderMatchType {
    #[default]
    Exact,
    RegularExpression,
}

impl HttpHeaderMatchType {
    fn is_default(&self) -> bool {
        *self == HttpHeaderMatchType::Exact
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpHeaderName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    String,
);

impl From<&HttpHeaderName> for HeaderName {
    fn from(name: &HttpHeaderName) -> Self {
        Self::from_bytes(name.0.as_bytes()).expect("Invalid header name")
    }
}

#[derive(
    Validate, Builder, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
pub struct HttpQueryParamMatch {
    #[getset(get = "pub")]
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "HttpQueryParamMatchType::is_default"
    )]
    match_type: HttpQueryParamMatchType,

    #[getset(get = "pub")]
    name: HttpQueryParamName,

    #[getset(get = "pub")]
    #[validate(min_length = 1)]
    #[validate(max_length = 1024)]
    value: String,
}

#[derive(Default, Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpQueryParamMatchType {
    #[default]
    Exact,
    RegularExpression,
}

impl HttpQueryParamMatchType {
    fn is_default(&self) -> bool {
        *self == HttpQueryParamMatchType::Exact
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpQueryParamName(
    #[validate(min_length = 1)]
    #[validate(max_length = 256)]
    #[validate(pattern = "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+$")]
    #[getset(get = "pub")]
    String,
);

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, EnumString)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethodMatch {
    #[strum(serialize = "GET")]
    Get,
    #[strum(serialize = "POST")]
    Post,
    #[strum(serialize = "PUT")]
    Put,
    #[strum(serialize = "PATCH")]
    Patch,
    #[strum(serialize = "DELETE")]
    Delete,
    #[strum(serialize = "HEAD")]
    Head,
    #[strum(serialize = "OPTIONS")]
    Options,
    #[strum(serialize = "TRACE")]
    Trace,
    #[strum(serialize = "CONNECT")]
    Connect,
}

impl Into<Method> for HttpMethodMatch {
    fn into(self) -> Method {
        match self {
            HttpMethodMatch::Get => Method::GET,
            HttpMethodMatch::Post => Method::POST,
            HttpMethodMatch::Put => Method::PUT,
            HttpMethodMatch::Patch => Method::PATCH,
            HttpMethodMatch::Delete => Method::DELETE,
            HttpMethodMatch::Head => Method::HEAD,
            HttpMethodMatch::Options => Method::OPTIONS,
            HttpMethodMatch::Trace => Method::TRACE,
            HttpMethodMatch::Connect => Method::CONNECT,
        }
    }
}
