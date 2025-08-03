mod constants;
mod context;
pub mod filters;
pub mod router;

use crate::proxy::context::{MatchRouteResult, UpstreamPeerResult};
use crate::proxy::router::endpoints::EndpointsResolver;
use async_trait::async_trait;
use bytes::Bytes;
use context::Context;
use filters::client_addrs::ClientAddrFilter;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE, SERVER};
use http::{HeaderName, HeaderValue, StatusCode};
use kubera_core::sync::signal::Receiver;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::{gen_error_response, HTTP_400_RESPONSE};
use router::HttpRouter;
use tracing::warn;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct Proxy {
    router_rx: Receiver<HttpRouter>,
    client_addr_filter_rx: Receiver<ClientAddrFilter>,
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = Context;

    fn new_ctx(&self) -> Self::CTX {
        Context::default()
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        match ctx.next_upstream_peer() {
            UpstreamPeerResult::Addr(addr) => {
                Ok(Box::new(HttpPeer::new(addr, false, "".to_string())))
            }
            UpstreamPeerResult::NotFound => Err(Error::explain(
                HTTPStatus(StatusCode::NOT_FOUND.into()),
                "No matching route found",
            )),
            UpstreamPeerResult::ServiceUnavailable => Err(Error::explain(
                HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                "Service unavailable",
            )),
            UpstreamPeerResult::MissingConfiguration => Err(Error::explain(
                HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                "Missing configuration",
            )),
        }
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let client_addr = if let Some(client_addr_filter) = self.client_addr_filter_rx.get().await {
            client_addr_filter.filter(session)
        } else {
            warn!("No client address filter configured");
            None
        };

        let router = self.router_rx.get().await;
        let route = if let Some(router) = router {
            let req_parts = session.req_header();
            match router.match_route(req_parts) {
                Some((route, rule)) => MatchRouteResult::Found(route, rule),
                None => MatchRouteResult::NotFound,
            }
        } else {
            MatchRouteResult::MissingConfiguration
        };

        let (status_code, msg) = match route {
            MatchRouteResult::Found(route, rule) => {
                ctx.set(MatchRouteResult::Found(route, rule), client_addr);
                return Ok(false);
            }
            MatchRouteResult::NotFound => (StatusCode::NOT_FOUND, "No matching route found"),
            MatchRouteResult::MissingConfiguration => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Gateway configuration missing",
            ),
        };
        let body = Bytes::from(format!(
            "<html><body><h1>{msg}</h1></body></html>",
        ));

        let mut response = gen_error_response(status_code.into());
        self.set_response_server_header(&mut response)?;
        response.insert_header(CONTENT_TYPE, "text/html")?;
        response.insert_header(CONTENT_LENGTH, body.len().to_string())?;

        session.write_response_header_ref(&response).await?;
        session.write_response_body(Some(body), true).await?;

        Ok(true)
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
        self.set_response_server_header(_upstream_response)?;
        Ok(())
    }
}

impl Proxy {
    fn set_response_server_header(&self, response: &mut ResponseHeader) -> Result<(), BError> {
        response.insert_header(SERVER, "Kubera Gateway")?;
        Ok(())
    }
}
