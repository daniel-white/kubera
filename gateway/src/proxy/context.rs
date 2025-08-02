use std::net::IpAddr;
use crate::proxy::router::{HttpRoute, HttpRouteRule};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Default)]
pub struct Context {
    state: OnceLock<(MatchRouteResult, Option<IpAddr>)>,
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
    
    pub fn route(&self) -> Option<&MatchRouteResult> {
        self.state.get().map(|x| &(*x).0)
    }
    
    pub fn client_addr(&self) -> Option<IpAddr> {
        self.state.get().and_then(|x| x.1)
    }
    
    pub fn set(&self, route: MatchRouteResult, client_addr: Option<IpAddr>) {
        let _ = self.state.set((route, client_addr));
    }
}
