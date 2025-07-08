use crate::proxy::router::{HttpRoute, HttpRouteRule, HttpRouter};
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub struct Context {
    router: Receiver<Option<HttpRouter>>,
    route: OnceLock<MatchRouteResult>,
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchRouteResult {
    Found(Arc<HttpRoute>, Arc<HttpRouteRule>),
    NotFound,
    MissingConfiguration,
}

impl Context {
    pub fn new(router: Receiver<Option<HttpRouter>>) -> Self {
        Self {
            router,
            route: OnceLock::new(),
        }
    }

    pub fn set_route(&self, parts: &Parts) -> &MatchRouteResult {
        self.route
            .get_or_init(|| match self.router.current().as_ref() {
                None => MatchRouteResult::MissingConfiguration,
                Some(router) => match router.match_route(parts) {
                    Some((route, rule)) => MatchRouteResult::Found(route, rule),
                    _ => MatchRouteResult::NotFound,
                },
            })
    }

    pub fn route(&self) -> Option<&MatchRouteResult> {
        self.route.get()
    }
}
