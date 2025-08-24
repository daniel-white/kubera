use crate::http::filters::access_control::{HttpAccessControlFilter, HttpAccessControlFilterRef};
use crate::http::filters::client_addrs::{HttpClientAddrsFilter, HttpClientAddrsFilterRef};
use crate::http::filters::error_response::{HttpErrorResponseFilter, HttpErrorResponseFilterRef};
use crate::http::filters::header_modifier::HttpHeaderModifierFilter;
use crate::http::filters::redirect::HttpRedirectFilter;
use crate::http::filters::static_response::HttpStaticResponseFilter;
use crate::http::routes::{HttpRoute, HttpRouteBuilder, HttpRouteKey};
use crate::net::Port;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HttpListenerProtocol {
    Http,
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpListener {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    port: Port,

    #[getset(get = "pub")]
    filters: Vec<HttpListenerFilter>,

    #[getset(get = "pub")]
    filter_definitions: Vec<HttpFilterDefinition>,

    #[getset(get = "pub")]
    routes: Vec<HttpRoute>,
}

impl HttpListener {
    pub fn builder() -> HttpListenerBuilder {
        HttpListenerBuilder {
            name: None,
            port: None,
            filters: Vec::new(),
            filter_definitions: Vec::new(),
            route_builders: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpListenerBuilder {
    name: Option<String>,
    port: Option<Port>,
    filters: Vec<HttpListenerFilter>,
    filter_definitions: Vec<HttpFilterDefinition>,
    route_builders: Vec<HttpRouteBuilder>,
}

impl HttpListenerBuilder {
    pub fn build(self) -> HttpListener {
        HttpListener {
            name: self.name.expect("Listener name is not set"),
            port: self.port.expect("Listener port is not set"),
            filters: self.filters,
            filter_definitions: self.filter_definitions,
            routes: self
                .route_builders
                .into_iter()
                .map(HttpRouteBuilder::build)
                .collect(),
        }
    }

    pub fn name<N: Into<String>>(&mut self, name: N) -> &mut Self {
        let name = name.into();
        self.name = Some(name);
        self
    }

    pub fn port<P: Into<Port>>(&mut self, port: P) -> &mut Self {
        let port = port.into();
        self.port = Some(port);
        self
    }

    pub fn add_filter(&mut self, filter: HttpListenerFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }

    pub fn add_filter_definition(&mut self, filter_definition: HttpFilterDefinition) -> &mut Self {
        self.filter_definitions.push(filter_definition);
        self
    }

    pub fn add_route<K, F>(&mut self, key: K, factory: F) -> &mut Self
    where
        K: Into<HttpRouteKey>,
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut route_builder = HttpRoute::builder(key);
        factory(&mut route_builder);
        self.route_builders.push(route_builder);
        self
    }
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HttpListenerFilter {
    UpstreamRequestHeaderModifier(HttpHeaderModifierFilter),
    ResponseHeaderModifier(HttpHeaderModifierFilter),
    Redirect(HttpRedirectFilter),
    AccessControl(HttpAccessControlFilterRef),
    ClientAddrs(HttpClientAddrsFilterRef),
    ErrorResponse(HttpErrorResponseFilterRef),
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HttpFilterDefinition {
    UpstreamRequestHeaderModifier(HttpHeaderModifierFilter),
    ResponseHeaderModifier(HttpHeaderModifierFilter),
    Redirect(HttpRedirectFilter),
    StaticResponse(HttpStaticResponseFilter),
    AccessControl(HttpAccessControlFilter),
    ClientAddrs(HttpClientAddrsFilter),
    ErrorResponse(HttpErrorResponseFilter),
}
