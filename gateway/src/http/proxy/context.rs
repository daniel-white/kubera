use crate::http::router::{Route, Router};
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Context {
    router: Receiver<Option<Router>>,
    route: OnceLock<FindRouteResult>,
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

#[derive(Debug, Clone, PartialEq)]
pub enum FindRouteResult {
    Found(Route),
    NotFound,
    MissingConfiguration,
}

impl Context {
    pub fn new(router: Receiver<Option<Router>>) -> Self {
        Self {
            router,
            route: OnceLock::new(),
        }
    }

    pub fn find_route(&self, parts: &Parts) -> &FindRouteResult {
        self.route.get_or_init(|| match self.router.current() {
            None => FindRouteResult::MissingConfiguration,
            Some(router) => match router.match_route(parts) {
                Some(route) => FindRouteResult::Found(route.clone()),
                _ => FindRouteResult::NotFound,
            },
        })
    }
}
