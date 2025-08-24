use super::backends::{HttpBackend, HttpBackendBuilder};
use crate::http::filters::access_control::HttpAccessControlFilterRef;
use crate::http::filters::client_addrs::HttpClientAddrsFilterRef;
use crate::http::filters::error_response::HttpErrorResponseFilterRef;
use crate::http::filters::header_modifier::HttpHeaderModifierFilter;
use crate::http::filters::redirect::HttpRedirectFilter;
use crate::http::filters::static_response::HttpStaticResponseFilterRef;
use crate::http::filters::uri_rewrite::HttpUpstreamUriRewrite;
use crate::http::matches::{HttpRequestMatches, HttpRouteRuleMatchesBuilder};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum HttpRouteRuleFilter {
    UpstreamUriRewrite(HttpUpstreamUriRewrite),
    UpstreamRequestHeaderModifier(HttpHeaderModifierFilter),
    Redirect(HttpRedirectFilter),
    ResponseHeaderModifier(HttpHeaderModifierFilter),
    StaticResponse(HttpStaticResponseFilterRef),
    AccessControl(HttpAccessControlFilterRef),
    ClientAddrs(HttpClientAddrsFilterRef),
    ErrorResponse(HttpErrorResponseFilterRef),
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
pub struct HttpRouteRuleKey(String);

impl<S: AsRef<str>> From<S> for HttpRouteRuleKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteRule {
    #[getset(get = "pub")]
    key: HttpRouteRuleKey,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    matches: Vec<HttpRequestMatches>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backends: Vec<HttpBackend>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    filters: Vec<HttpRouteRuleFilter>,
}

impl HttpRouteRule {
    pub fn builder<K: Into<HttpRouteRuleKey>>(key: K) -> HttpRouteRuleBuilder {
        HttpRouteRuleBuilder {
            key: key.into(),
            match_builders: Vec::new(),
            backend_builders: Vec::new(),
            filters: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpRouteRuleBuilder {
    key: HttpRouteRuleKey,
    match_builders: Vec<HttpRouteRuleMatchesBuilder>,
    backend_builders: Vec<HttpBackendBuilder>,
    filters: Vec<HttpRouteRuleFilter>,
}

impl HttpRouteRuleBuilder {
    pub fn build(self) -> HttpRouteRule {
        HttpRouteRule {
            key: self.key,
            matches: self
                .match_builders
                .into_iter()
                .map(HttpRouteRuleMatchesBuilder::build)
                .collect(),
            backends: self
                .backend_builders
                .into_iter()
                .map(HttpBackendBuilder::build)
                .collect(),
            filters: self.filters,
        }
    }

    pub fn add_match<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleMatchesBuilder),
    {
        let mut builder = HttpRequestMatches::builder();
        factory(&mut builder);
        self.match_builders.push(builder);
        self
    }

    pub fn add_backend<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpBackendBuilder),
    {
        let mut builder = HttpBackend::builder();
        factory(&mut builder);
        self.backend_builders.push(builder);
        self
    }

    pub fn add_filter(&mut self, filter: HttpRouteRuleFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }
}
