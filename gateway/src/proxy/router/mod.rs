pub mod endpoints;
mod matches;
mod routes;
pub mod topology;

use crate::proxy::router::matches::{HostMatch, HostValueMatch};
use crate::proxy::router::routes::HttpRouteBuilder;
use crate::proxy::router::topology::{TopologyLocation, TopologyLocationMatch};
use enumflags2::BitFlags;
use getset::{CopyGetters, Getters};
use http::request::Parts;
use itertools::Itertools;
pub use matches::HttpRouteRuleMatches;
pub use routes::HttpRoute;
pub use routes::HttpRouteRule;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;
use vg_core::net::Hostname;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HttpRouter {
    host_matches: HostMatch,
    routes: Vec<Arc<HttpRoute>>,
}

/// Enhanced router match result that includes matched prefix context
#[derive(Debug, Clone)]
pub enum HttpRouterMatchResult {
    Matched {
        route: Arc<HttpRoute>,
        rule: Arc<HttpRouteRule>,
        matched_prefix: Option<String>,
    },
    #[allow(dead_code)] // Future use for non-matched scenarios
    NotMatched,
}

impl HttpRouterMatchResult {
    pub fn new(
        route: Arc<HttpRoute>,
        rule: Arc<HttpRouteRule>,
        matched_prefix: Option<String>,
    ) -> Self {
        Self::Matched {
            route,
            rule,
            matched_prefix,
        }
    }

    /// Check if the result represents a match
    #[allow(dead_code)] // Public API for match checking
    pub fn is_matched(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    /// Get the route if matched
    pub fn route(&self) -> Option<&Arc<HttpRoute>> {
        match self {
            Self::Matched { route, .. } => Some(route),
            Self::NotMatched => None,
        }
    }

    /// Get the rule if matched
    pub fn rule(&self) -> Option<&Arc<HttpRouteRule>> {
        match self {
            Self::Matched { rule, .. } => Some(rule),
            Self::NotMatched => None,
        }
    }

    /// Get the matched prefix if available
    pub fn matched_prefix(&self) -> Option<&String> {
        match self {
            Self::Matched { matched_prefix, .. } => matched_prefix.as_ref(),
            Self::NotMatched => None,
        }
    }
}

pub struct HttpRouterBuilder {
    current_location: Arc<TopologyLocation>,
    host_value_matches: Vec<HostValueMatch>,
    routes_builders: Vec<HttpRouteBuilder>,
}

impl HttpRouterBuilder {
    pub fn new(current_location: Arc<TopologyLocation>) -> Self {
        HttpRouterBuilder {
            current_location,
            host_value_matches: Vec::new(),
            routes_builders: Vec::new(),
        }
    }

    pub fn build(self) -> HttpRouter {
        let hosts = HostMatch {
            host_value_matches: self.host_value_matches,
        };

        HttpRouter {
            host_matches: hosts,
            routes: self
                .routes_builders
                .into_iter()
                .map(|b| Arc::new(b.build()))
                .collect(),
        }
    }

    pub fn add_route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut builder = HttpRouteBuilder::new(&self.current_location);
        factory(&mut builder);
        self.routes_builders.push(builder);

        self
    }

    #[allow(dead_code)] // Public API for exact host matching
    pub fn add_exact_host(&mut self, host: &Hostname) -> &mut Self {
        let host_value_match = HostValueMatch::Exact(host.clone());
        self.host_value_matches.push(host_value_match);
        self
    }

    #[allow(dead_code)] // Public API for host suffix matching
    pub fn add_host_suffix(&mut self, host: &Hostname) -> &mut Self {
        let host_value_match = HostValueMatch::Suffix(host.clone());
        self.host_value_matches.push(host_value_match);
        self
    }
}

impl HttpRouter {
    #[instrument("match_route", skip(self, parts))]
    pub fn match_route(&self, parts: &Parts) -> Option<HttpRouterMatchResult> {
        if !self.host_matches.matches(&parts.headers) {
            return None;
        }

        self.routes
            .iter()
            .enumerate()
            .filter_map(|(i, route)| {
                let match_result = route.matches(parts);
                if match_result.is_matched() {
                    debug!("Route {} matched with rule", i);
                    Some((i, route, match_result))
                } else {
                    None
                }
            })
            .min_by(|(_, _, lhs), (_, _, rhs)| lhs.score().unwrap().cmp(rhs.score().unwrap()))
            .map(|(i, route, match_result)| {
                debug!("Returning matched route at index {}", i);
                HttpRouterMatchResult::new(
                    route.clone(),
                    match_result.rule().unwrap().clone(),
                    match_result.matched_prefix().cloned(),
                )
            })
    }
}

#[derive(Getters, Debug, Clone, PartialEq, Eq)]
pub struct HttpBackend {
    #[getset(get = "pub")]
    weight: i32,

    #[getset(get = "pub")]
    endpoints: HashMap<BitFlags<TopologyLocationMatch>, Vec<HttpBackendEndpoint>>,
}

pub struct HttpBackendBuilder {
    current_location: Arc<TopologyLocation>,
    weight: i32,
    port: Option<u16>,
    endpoints: Vec<(TopologyLocation, HttpBackendEndpoint)>,
}

impl HttpBackendBuilder {
    pub fn new(current_location: &Arc<TopologyLocation>) -> Self {
        HttpBackendBuilder {
            current_location: current_location.clone(),
            weight: 1,
            port: None,
            endpoints: Vec::new(),
        }
    }

    pub fn build(self) -> HttpBackend {
        // Apply the port to all endpoints if not already set
        let port = self.port.unwrap_or(80);
        let endpoints: HashMap<_, _> = self
            .endpoints
            .into_iter()
            .map(|(location, mut endpoint)| {
                // Overwrite the port in the endpoint's SocketAddr
                let addr = SocketAddr::new(endpoint.addr.ip(), port);
                endpoint.addr = addr;
                let score = TopologyLocationMatch::matches(&self.current_location, &location);
                let score = if score.contains(TopologyLocationMatch::Node) {
                    BitFlags::from(TopologyLocationMatch::Node)
                } else if score.contains(TopologyLocationMatch::Zone) {
                    BitFlags::from(TopologyLocationMatch::Zone)
                } else {
                    BitFlags::empty()
                };
                (score, endpoint)
            })
            .into_group_map();

        HttpBackend {
            weight: self.weight,
            endpoints,
        }
    }

    pub fn with_weight(&mut self, weight: i32) -> &mut Self {
        self.weight = weight;
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = Some(port);
        self
    }

    pub fn add_endpoint(&mut self, ip_addr: IpAddr, location: TopologyLocation) -> &mut Self {
        let endpoint = HttpBackendEndpoint::builder()
            .addr(SocketAddr::new(ip_addr, 0))
            .build();
        self.endpoints.push((location, endpoint));
        self
    }
}

#[derive(CopyGetters, Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct HttpBackendEndpoint {
    #[getset(get_copy = "pub")]
    addr: SocketAddr,
}
