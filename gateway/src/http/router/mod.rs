mod matches;
mod routes;

use crate::http::router::matches::{HostMatch, HostValueMatch};
use derive_builder::Builder;
use getset::Getters;
use http::request::Parts;
pub use matches::{HttpRouteRuleMatches, HttpRouteRuleMatchesResult};
use std::net::IpAddr;

use crate::http::router::routes::{HttpRoute, HttpRouteBuilder};
use tracing::{debug, instrument};
use kubera_core::config::gateway::types::CaseInsensitiveString;
use kubera_core::config::gateway::types::net::Hostname;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HttpRouter {
    host_matches: HostMatch,
    routes: Vec<HttpRoute>,
}

#[derive(Default)]
pub struct HttpRouterBuilder {
    host_value_matches: Vec<HostValueMatch>,
    routes_builders: Vec<HttpRouteBuilder>,
}

impl HttpRouterBuilder {
    pub fn build(self) -> HttpRouter {
        let hosts = HostMatch {
            host_value_matches: self.host_value_matches,
        };

        HttpRouter {
            host_matches: hosts,
            routes: self
                .routes_builders
                .into_iter()
                .map(|b| b.build())
                .collect(),
        }
    }

    pub fn add_route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut builder = HttpRoute::new_builder();
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
    pub fn new_builder() -> HttpRouterBuilder {
        HttpRouterBuilder::default()
    }

    pub fn match_route(&self, parts: &Parts) -> Option<&HttpRoute> {
        None
        // self.routes
        //     .iter()
        //     .enumerate()
        //     .filter_map(|(i, r)| match r.matches(parts) {
        //         HttpRouteRuleMatchesResult::Matched(score) => {
        //             debug!("Matched route at index {} with score: {:?}", i, score);
        //             Some((i, r, score))
        //         }
        //         HttpRouteRuleMatchesResult::NotMatched => {
        //             debug!("Route at index {} did not match", i);
        //             None
        //         }
        //     })
        //     .min_by(|(_, _, lhs), (_, _, rhs)| lhs.cmp(rhs))
        //     .map(|(i, r, _)| {
        //         debug!("Returning matched route at index {}", i);
        //         r
        //     })
    }
}

#[derive(Getters, Debug, Clone, PartialEq, Eq)]
pub struct HttpBackend {
    #[getset(get = "pub")]
    weight: i32,

    #[getset(get = "pub")]
    port: Option<u16>,

    #[getset(get = "pub")]
    endpoints: Vec<HttpBackendEndpoint>,
}

impl HttpBackend {
    pub fn new_builder() -> HttpBackendBuilder {
        HttpBackendBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct HttpBackendBuilder {
    weight: i32,
    port: Option<u16>,
    endpoint_builders: Vec<HttpBackendEndpointBuilder>,
}

impl HttpBackendBuilder {
    pub fn build(self) -> HttpBackend {
        HttpBackend {
            weight: self.weight,
            port: self.port,
            endpoints: self
                .endpoint_builders
                .into_iter()
                .map(|b| b.build())
                .collect(),
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
        F: FnOnce(&mut HttpBackendEndpointBuilder) -> (),
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
    node: Option<String>,

    #[getset(get = "pub")]
    zone: Option<String>,

    #[getset(get = "pub")]
    address: IpAddr,
}

#[derive(Debug)]
pub struct HttpBackendEndpointBuilder {
    node: Option<String>,
    zone: Option<String>,
    address: IpAddr,
}

impl HttpBackendEndpointBuilder {
    pub fn build(self) -> HttpBackendEndpoint {
        HttpBackendEndpoint {
            node: self.node,
            zone: self.zone,
            address: self.address,
        }
    }

    pub fn for_address(address: IpAddr) -> Self {
        HttpBackendEndpointBuilder {
            node: None,
            zone: None,
            address,
        }
    }

    pub fn on_node(&mut self, node: String) -> &mut Self {
        self.node = Some(node);
        self
    }

    pub fn in_zone(&mut self, zone: String) -> &mut Self {
        self.zone = Some(zone);
        self
    }
}
