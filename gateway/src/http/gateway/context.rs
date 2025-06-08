use crate::config::routes_controller::Routes;
use crate::http::router::{MatchResult, Route};
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Context {
    routes: Receiver<Routes>,
    route: OnceLock<Option<Route>>,
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

impl Context {
    pub fn new(routes: Receiver<Routes>) -> Self {
        Self {
            routes,
            route: OnceLock::new(),
        }
    }

    pub fn route(&self, parts: &Parts) -> Option<&Route> {
        self.route
            .get_or_init(|| {
                let routes = self.routes.current();
                let routes = routes.get();
                routes
                    .iter()
                    .filter_map(|r| match r.matches(parts) {
                        MatchResult::Matched(score) => Some((r, score)),
                        MatchResult::NotMatched => None,
                    })
                    .min_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs))
                    .map(|(r, _)| r)
                    .cloned()
            })
            .as_ref()
    }
}
