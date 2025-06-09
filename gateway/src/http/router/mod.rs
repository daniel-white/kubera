mod matchers;
mod route;

use crate::http::router::matchers::RouteMatcherBuilder;
use derive_builder::Builder;
use http::request::Parts;
use tracing::error;
pub use matchers::{MatchResult, RouteMatcher};
pub use route::Route;

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
            .filter_map(|r| match r.matches(parts) {
                MatchResult::Matched(score) => Some((r, score)),
                MatchResult::NotMatched => None,
            })
            .min_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs))
            .map(|(r, _)| r)
    }
}
