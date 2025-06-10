mod context;

use crate::http::router::Router;
use crate::net::resolver::SocketAddrResolver;
use async_trait::async_trait;
use context::Context;
use derive_builder::Builder;
use http::HeaderValue;
use kubera_core::sync::signal::Receiver;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use tracing::warn;

#[derive(Debug, Builder)]
pub struct Proxy {
    router: Receiver<Option<Router>>,
    addr_resolver: SocketAddrResolver,
}

#[async_trait]
impl ProxyHttp for Proxy {
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
                let upstreams = route.upstreams();
                let resolved = upstreams.iter().map(|u| self.addr_resolver.resolve(u));

                warn!("Found route: {:?}", resolved);
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

    async fn response_filter(
        &self,
        _session: &mut Session,
        _upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        warn!("Response filter is not implemented yet");
        let _ = _upstream_response.insert_header(
            http_constant::SERVER,
            HeaderValue::from_str("Kubera Gateway").unwrap(),
        );
        Ok(())
    }
}
