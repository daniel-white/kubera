pub mod endpoints;
mod matches;
mod routes;
pub mod topology;

use crate::proxy::router::matches::{HostMatch, HostValueMatch};
use crate::proxy::router::routes::{HttpRouteBuilder, HttpRouteMatchResult};
use crate::proxy::router::topology::{
    TopologyLocation, TopologyLocationBuilder, TopologyLocationMatch,
};
use enumflags2::BitFlags;
use getset::Getters;
use http::request::Parts;
use itertools::Itertools;
use kubera_core::net::Hostname;
pub use matches::HttpRouteRuleMatches;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::debug;

pub use routes::HttpRoute;
pub use routes::HttpRouteRule;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HttpRouter {
    host_matches: HostMatch,
    routes: Vec<Arc<HttpRoute>>,
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

    pub fn add_exact_host(&mut self, host: &Hostname) -> &mut Self {
        let host_value_match = HostValueMatch::Exact(host.clone());
        self.host_value_matches.push(host_value_match);
        self
    }

    pub fn add_host_suffix(&mut self, host: &Hostname) -> &mut Self {
        let host_value_match = HostValueMatch::Suffix(host.clone());
        self.host_value_matches.push(host_value_match);
        self
    }
}

impl HttpRouter {
    pub fn match_route(&self, parts: &Parts) -> Option<(Arc<HttpRoute>, Arc<HttpRouteRule>)> {
        self.routes
            .iter()
            .enumerate()
            .filter_map(|(i, route)| match route.matches(parts) {
                HttpRouteMatchResult::Matched(rule, score) => Some((i, route, rule, score)),
                HttpRouteMatchResult::NotMatched => {
                    debug!("Route at index {} did not match", i);
                    None
                }
            })
            .min_by(|(_, _, _, lhs), (_, _, _, rhs)| lhs.cmp(rhs))
            .map(|(i, route, rule, _)| {
                debug!("Returning matched route at index {}", i);
                (route.clone(), rule.clone())
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

#[derive(Debug)]
pub struct HttpBackendBuilder {
    current_location: Arc<TopologyLocation>,
    weight: i32,
    port: Option<u16>,
    endpoint_builders: Vec<HttpBackendEndpointBuilder>,
}

impl HttpBackendBuilder {
    pub fn new(current_location: &Arc<TopologyLocation>) -> Self {
        HttpBackendBuilder {
            current_location: current_location.clone(),
            weight: 1,
            port: None,
            endpoint_builders: Vec::new(),
        }
    }

    pub fn build(self) -> HttpBackend {
        let endpoints: HashMap<_, _> = self
            .endpoint_builders
            .into_iter()
            .map(|b| {
                let (location, endpoint) = b.build();
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

    pub fn add_endpoint<F>(&mut self, address: IpAddr, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpBackendEndpointBuilder),
    {
        let mut endpoint_builder = HttpBackendEndpointBuilder::for_address(address);
        factory(&mut endpoint_builder);
        self.endpoint_builders.push(endpoint_builder);
        self
    }
}

#[derive(Getters, Debug, Clone, PartialEq, Eq)]
pub struct HttpBackendEndpoint {
    #[getset(get = "pub")]
    address: IpAddr,
}

#[derive(Debug)]
pub struct HttpBackendEndpointBuilder {
    location_builder: TopologyLocationBuilder,
    address: IpAddr,
}

impl HttpBackendEndpointBuilder {
    pub fn build(self) -> (TopologyLocation, HttpBackendEndpoint) {
        let location = self.location_builder.build();
        let endpoint = HttpBackendEndpoint {
            address: self.address,
        };

        (location, endpoint)
    }

    pub fn for_address(address: IpAddr) -> Self {
        HttpBackendEndpointBuilder {
            location_builder: TopologyLocation::new_builder(),
            address,
        }
    }

    pub fn located<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut TopologyLocationBuilder),
    {
        let mut builder = TopologyLocation::new_builder();
        factory(&mut builder);
        self.location_builder = builder;
        self
    }
}
