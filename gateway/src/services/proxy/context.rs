use crate::services::proxy::router::HttpRouter;
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Context {
    router: Receiver<Option<HttpRouter>>,
    route: OnceLock<FindRouteResult>,
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

#[derive(Debug, Clone, PartialEq)]
pub enum FindRouteResult {
    Found(()),
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

    pub fn find_route(&self, parts: &Parts) -> &FindRouteResult {
        &FindRouteResult::NotFound
        // self.route
        //     .get_or_init(|| match self.router.current().as_ref() {
        //         None => FindRouteResult::MissingConfiguration,
        //         Some(router) => match router.match_route(parts) {
        //             Some(route) => FindRouteResult::Found(route.clone()),
        //             _ => FindRouteResult::NotFound,
        //         },
        //     })
    }
}
