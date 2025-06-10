use super::matchers::{MatchResult, RouteMatcher};
use super::upstreams::Upstream;
use derive_builder::Builder;
use getset::Getters;
use http::request::Parts;
use tracing::instrument;

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

    #[instrument(skip(self, parts), level = "debug", name = "Route::matches")]
    pub fn matches(&self, parts: &Parts) -> MatchResult {
        self.matcher.matches(parts)
    }
}
