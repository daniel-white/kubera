use super::matchers::{MatchResult, RouteMatcher};
use derive_builder::Builder;
use getset::Getters;
use http::request::Parts;
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq)]
pub enum TransportSecurity {
    None,
    Tls,
}

#[derive(Debug, Builder, Getters, Clone, PartialEq)]
pub struct Upstream {
    #[getset(get = "pub")]
    address: (), // TODO: replace with actual type
    #[getset(get = "pub")]
    transport_security: TransportSecurity,
}

impl Upstream {
    pub fn new_builder() -> UpstreamBuilder {
        UpstreamBuilder::default()
    }
}

#[derive(Debug, Builder, Getters, Clone, PartialEq)]
pub struct Route {
    matcher: RouteMatcher,
    #[getset(get = "pub")]
    upstreams: Vec<Upstream>,
}

impl Route {
    pub fn new_builder() -> RouteBuilder {
        RouteBuilder::default()
    }

    pub fn matches(&self, parts: &Parts) -> MatchResult {
        self.matcher.matches(parts)
    }
}
