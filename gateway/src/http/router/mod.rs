mod matchers;
mod route;
mod upstreams;

use crate::http::router::matchers::RouteMatcherBuilder;
use derive_builder::Builder;
use http::request::Parts;
pub use matchers::{MatchResult, RouteMatcher};
pub use route::Route;
use tracing::{debug, error};
pub use upstreams::Upstream;
use upstreams::UpstreamsBuilder;

#[derive(Debug, Builder, Clone, Default, PartialEq)]
pub struct Router {
    routes: Vec<Route>,
}

impl RouterBuilder {
    pub fn route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut RouteMatcherBuilder, &mut UpstreamsBuilder),
    {
        let mut route_builder = Route::new_builder();
        let mut matcher_builder = RouteMatcher::new_builder();
        let mut upstreams_builder = UpstreamsBuilder::new();

        factory(&mut matcher_builder, &mut upstreams_builder);

        let matcher = matcher_builder.build();
        let upstreams = upstreams_builder.build();
        route_builder.matcher(matcher);
        route_builder.upstreams(upstreams);

        match route_builder.build() {
            Ok(route) => {
                let routes = self.routes.get_or_insert_default();
                routes.push(route);
                debug!("Added new route, total routes: {}", routes.len());
            }
            Err(e) => {
                error!("Failed to build route: {}", e);
            }
        }

        self
    }
}

impl Router {
    pub fn new_builder() -> RouterBuilder {
        RouterBuilder::default()
    }

    pub fn match_route(&self, parts: &Parts) -> Option<&Route> {
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
