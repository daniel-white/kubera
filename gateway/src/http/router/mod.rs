mod matchers;
mod route;

use crate::http::router::matchers::RouteMatcherBuilder;
use derive_builder::Builder;
use http::request::Parts;
pub use matchers::{MatchResult, RouteMatcher};
pub use route::Route;
use tracing::field::debug;
use tracing::{debug, error};

#[derive(Debug, Builder, Clone, Default, PartialEq)]
pub struct Router {
    routes: Vec<Route>,
}

impl RouterBuilder {
    pub fn route<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut RouteMatcherBuilder),
    {
        let mut matcher_builder = RouteMatcher::new_builder();
        factory(&mut matcher_builder);
        let matcher = matcher_builder.build();

        let mut route_builder = Route::new_builder();
        route_builder.upstreams(vec![]).matcher(matcher);

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
