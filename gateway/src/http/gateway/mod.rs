mod context;

use crate::http::router::Router;
use async_trait::async_trait;
use context::Context;
use derive_builder::Builder;
use http::request::Parts;
use kubera_core::sync::signal::Receiver;
use pingora::prelude::*;
use std::ops::DerefMut;
use tracing::Instrument;

#[derive(Debug, Builder)]
pub struct Gateway {
    router: Receiver<Option<Router>>,
}

#[async_trait]
impl ProxyHttp for Gateway {
    type CTX = Context;

    fn new_ctx(&self) -> Self::CTX {
        Context::new(self.router.clone())
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        match _ctx.find_route(session.req_header()) {
            context::FindRouteResult::Found(route) => {
                Err(Error::explain(HTTPStatus(400), "Not implemented")) // TODO implement route to upstream
            }
            context::FindRouteResult::NotFound => {
                Err(Error::explain(HTTPStatus(404), "No matching route found"))
            }
            context::FindRouteResult::MissingConfiguration => {
                Err(Error::explain(HTTPStatus(503), "Missing configuration"))
            }
        }
    }
}
