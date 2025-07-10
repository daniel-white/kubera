mod constants;
mod context;
pub mod router;
pub mod filters;

use crate::proxy::context::MatchRouteResult;
use crate::proxy::router::endpoints::EndpointsResolver;
use crate::proxy::router::topology::TopologyLocation;
use async_trait::async_trait;
use context::Context;
use derive_builder::Builder;
use http::HeaderValue;
use kubera_core::sync::signal::Receiver;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use router::HttpRouter;
use std::net::SocketAddr;
use tracing::warn;
use filters::client_addrs::ClientAddrFilter;

#[derive(Debug, Builder)]
pub struct Proxy {
    router: Receiver<Option<HttpRouter>>,
    client_addr_filter: Receiver<ClientAddrFilter>
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = Context;

    fn new_ctx(&self) -> Self::CTX {
        Context::new(self.router.clone())
    }

    async fn early_request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        let client_addr_filter = self.client_addr_filter.current();
        
        client_addr_filter.filter(session);
        
        Ok(())
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        match _ctx.set_route(session.req_header()) {
            MatchRouteResult::Found(_, rule) => {
                let mut location = TopologyLocation::new_builder();
                location.on_node(Some("minikube".to_string()));
                let location = location.build();

                let mut endpoints = EndpointsResolver::new_builder(location.clone());
                for be in rule.backends() {
                    for addrs in be.endpoints().values() {
                        for addr in addrs {
                            endpoints
                                .insert(SocketAddr::new(*addr.address(), 80), location.clone());
                        }
                    }
                }

                let endpoints = endpoints.build();

                let ep: Vec<_> = endpoints.resolve(None).collect();

                Ok(Box::new(HttpPeer::new(ep[0], false, "".to_string())))

                //Err(Error::explain(HTTPStatus(400), "Not implemented")) // TODO implement route to upstream
            }
            MatchRouteResult::NotFound => {
                Err(Error::explain(HTTPStatus(404), "No matching route found"))
            }
            MatchRouteResult::MissingConfiguration => {
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
