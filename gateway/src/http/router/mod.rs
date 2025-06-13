mod matches;

use crate::http::router::matches::{HostMatch, HostValueMatch, HttpRouteMatchesBuilder};
use getset::Getters;
use http::request::Parts;
pub use matches::{HttpRouteMatches, MatchResult};

use tracing::{debug, instrument};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HttpRouter {
    hosts: HostMatch,
    routes: Vec<HttpRoute>,
}

#[derive(Default)]
pub struct HttpRouterBuilder {
    host_value_matches: Vec<HostValueMatch>,
    routes: Vec<HttpRoute>,
}

impl HttpRouterBuilder {
    pub fn build(self) -> HttpRouter {
        let hosts = HostMatch {
            host_value_matches: self.host_value_matches,
        };

        HttpRouter {
            hosts,
            routes: self.routes,
        }
    }

    pub fn add_route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut builder = HttpRoute::new_builder();
        factory(&mut builder);
        let route = builder.build();
        self.routes.push(route);

        self
    }

    pub fn with_exact_host(&mut self, host: &str) -> &mut Self {
        let host_value_match = HostValueMatch::Exact(host.into());
        self.host_value_matches.push(host_value_match);
        self
    }

    pub fn with_host_suffix(&mut self, host: &str) -> &mut Self {
        let host_value_match = HostValueMatch::Suffix(host.into());
        self.host_value_matches.push(host_value_match);
        self
    }
}

impl HttpRouter {
    pub fn new_builder() -> HttpRouterBuilder {
        HttpRouterBuilder::default()
    }

    pub fn match_route(&self, parts: &Parts) -> Option<&HttpRoute> {
        self.routes
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r.matches(parts) {
                MatchResult::Matched(score) => {
                    debug!("Matched route at index {} with score: {:?}", i, score);
                    Some((i, r, score))
                }
                MatchResult::NotMatched => {
                    debug!("Route at index {} did not match", i);
                    None
                }
            })
            .min_by(|(_, _, lhs), (_, _, rhs)| lhs.cmp(rhs))
            .map(|(i, r, _)| {
                debug!("Returning matched route at index {}", i);
                r
            })
    }
}

#[derive(Debug, Getters, Clone, PartialEq)]
pub struct HttpRoute {
    matches: HttpRouteMatches,
}

impl HttpRoute {
    pub fn new_builder() -> HttpRouteBuilder {
        HttpRouteBuilder::default()
    }

    #[instrument(skip(self, parts), level = "debug", name = "HttpRoute::matches")]
    pub fn matches(&self, parts: &Parts) -> MatchResult {
        self.matches.matches(parts)
    }
}

#[derive(Default)]
pub struct HttpRouteBuilder {
    matches_builder: HttpRouteMatchesBuilder,
}

impl HttpRouteBuilder {
    pub fn build(self) -> HttpRoute {
        let matches = self.matches_builder.build();

        HttpRoute { matches }
    }

    pub fn with_matches<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteMatchesBuilder),
    {
        factory(&mut self.matches_builder);
        self
    }
}
