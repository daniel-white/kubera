mod constants;
mod context;
pub mod filters;
pub mod responses;
pub mod router;

use crate::proxy::context::{MatchRouteResult, UpstreamPeerResult};
use crate::proxy::responses::error_responses::{ErrorResponseCode, ErrorResponseGenerators};
use async_trait::async_trait;
use context::Context;
use filters::client_addrs::ClientAddrFilter;
use http::StatusCode;
use http::header::SERVER;
use kubera_core::sync::signal::Receiver;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use router::HttpRouter;
use tracing::warn;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct Proxy {
    router_rx: Receiver<HttpRouter>,
    client_addr_filter_rx: Receiver<ClientAddrFilter>,
    error_responses_rx: Receiver<ErrorResponseGenerators>,
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = Context;

    fn new_ctx(&self) -> Self::CTX {
        Context::builder()
            .error_response_generators_rx(self.error_responses_rx.clone())
            .build()
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

        let error_code = match route {
            MatchRouteResult::Found(route, rule) => {
                ctx.set(MatchRouteResult::Found(route, rule), client_addr);
                return Ok(false);
            }
            MatchRouteResult::NotFound => ErrorResponseCode::NoRoute,
            MatchRouteResult::MissingConfiguration => ErrorResponseCode::MissingConfiguration,
        };
        let response = ctx.generate_error_response(error_code).await;

        let mut error_response = gen_error_response(response.status().into());
        self.set_response_server_header(&mut error_response)?;
        for (name, value) in response.headers() {
            error_response.insert_header(name, value)?;
        }

        session.write_response_header_ref(&error_response).await?;
        session
            .write_response_body(response.body().clone(), true)
            .await?;

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
