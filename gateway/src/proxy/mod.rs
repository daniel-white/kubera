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
use filters::request_headers::RequestHeaderFilter;
use filters::request_redirect::RequestRedirectFilter;
use filters::response_headers::ResponseHeaderFilter;
use http::header::SERVER;
use http::StatusCode;
use kubera_core::sync::signal::Receiver;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use router::HttpRouter;
use tracing::{debug, warn};
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
                Some(match_result) => MatchRouteResult::Found(
                    match_result.route().unwrap().clone(),
                    match_result.rule().unwrap().clone(),
                    match_result.matched_prefix().cloned(),
                ),
                None => MatchRouteResult::NotFound,
            }
        } else {
            MatchRouteResult::MissingConfiguration
        };

        let error_code = match route {
            MatchRouteResult::Found(route, rule, matched_prefix) => {
                // Check for redirect filters before proceeding to upstream
                for filter in rule.filters() {
                    if let Some(request_redirect) = &filter.request_redirect {
                        let redirect_filter = RequestRedirectFilter::new(request_redirect.clone());

                        // Create route match context with the matched prefix
                        let route_context =
                            crate::proxy::filters::request_redirect::RouteMatchContext {
                                matched_prefix: matched_prefix.clone(),
                            };

                        if let Ok(Some(redirect_response)) = redirect_filter
                            .apply_to_pingora_request_with_context(
                                session.req_header(),
                                &route_context,
                            )
                        {
                            debug!(
                                "Applying redirect filter for route: {:?} with prefix: {:?}",
                                route, matched_prefix
                            );

                            // Generate redirect response
                            let mut redirect_resp =
                                gen_error_response(redirect_response.status_code.as_u16());
                            self.set_response_server_header(&mut redirect_resp)?;
                            redirect_resp.insert_header("Location", &redirect_response.location)?;

                            session.write_response_header_ref(&redirect_resp).await?;
                            session
                                .write_response_body(Some(bytes::Bytes::new()), true)
                                .await?;

                            return Ok(true); // Request handled, don't proceed to upstream
                        }
                    }
                }

                ctx.set(
                    MatchRouteResult::Found(route, rule, matched_prefix),
                    client_addr,
                );
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
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        self.set_response_server_header(_upstream_response)?;

        // Apply response header modifications from the matched route rule
        if let Some(context::MatchRouteResult::Found(route, rule, _)) = ctx.route() {
            if !rule.filters().is_empty() {
                for filter in rule.filters() {
                    if let Some(response_header_modifier) = &filter.response_header_modifier {
                        let header_filter =
                            ResponseHeaderFilter::new(response_header_modifier.clone());
                        if let Err(e) = header_filter.apply_to_pingora_headers(_upstream_response) {
                            warn!("Failed to apply response header filter: {}", e);
                        } else {
                            debug!("Applied response header filter for route: {:?}", route);
                        }
                    }
                }
            }
        } else {
            debug!("No matched route found for response header filter");
        }

        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Apply backend-level header modifications from the matched route rule
        if let Some(context::MatchRouteResult::Found(route, rule, _)) = ctx.route() {
            if !rule.filters().is_empty() {
                for filter in rule.filters() {
                    if let Some(request_header_modifier) = &filter.request_header_modifier {
                        let header_filter =
                            RequestHeaderFilter::new(request_header_modifier.clone());
                        if let Err(e) = header_filter.apply_to_pingora_headers(upstream_request) {
                            warn!("Failed to apply upstream request header filter: {}", e);
                        } else {
                            debug!(
                                "Applied upstream request header filter for route: {:?}",
                                route
                            );
                        }
                    }
                }
            }
        } else {
            debug!("No matched route found for upstream request header filter");
        }

        Ok(())
    }
}

impl Proxy {
    fn set_response_server_header(&self, response: &mut ResponseHeader) -> Result<(), BError> {
        response.insert_header(SERVER, "Kubera Gateway")?;
        Ok(())
    }
}
