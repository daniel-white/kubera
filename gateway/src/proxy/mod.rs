mod constants;
mod context;
pub mod filters;
mod instrumentation;
pub mod responses;
pub mod router;

use crate::controllers::static_response_bodies_cache::StaticResponseBodiesCache;
use crate::proxy::context::{MatchRouteResult, UpstreamPeerResult};

use crate::proxy::filters::access_control::AccessControlFilterHandlers;
use crate::proxy::filters::static_responses::StaticResponseFilter;
use crate::proxy::instrumentation::RequestInstrumentation;
use crate::proxy::responses::error_responses::{ErrorResponseCode, ErrorResponseGenerators};
use async_trait::async_trait;
use context::RequestContext;
use filters::client_addrs::ClientAddrFilterHandler;
use filters::request_headers::RequestHeaderFilter;
use filters::request_redirect::RequestRedirectFilter;
use filters::response_headers::ResponseHeaderFilter;
use filters::url_rewrite::URLRewriteFilter;
use http::header::SERVER;
use http::{HeaderMap, StatusCode};
use itertools::Itertools;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use router::HttpRouter;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, instrument, warn};
use typed_builder::TypedBuilder;
use vg_core::config::gateway::types::net::StaticResponse;
use vg_core::sync::signal::Receiver;
use vg_core::{await_ready, ReadyState};

#[derive(TypedBuilder)]
pub struct Proxy {
    router_rx: Receiver<HttpRouter>,
    client_addr_filter_handler_rx: Receiver<ClientAddrFilterHandler>,
    access_control_filters_handlers_rx: Receiver<AccessControlFilterHandlers>,
    error_responses_rx: Receiver<ErrorResponseGenerators>,
    static_responses_rx: Receiver<Arc<HashMap<String, StaticResponse>>>,
    static_response_bodies_cache: StaticResponseBodiesCache,
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        let instrumentation = RequestInstrumentation::new();

        RequestContext::builder()
            .instrumentation(instrumentation)
            .error_response_generators_rx(self.error_responses_rx.clone())
            .build()
    }

    #[instrument(name = "upstream_peer", parent = ctx.instrumentation().request_span(), skip(self, _session, ctx))]
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        match ctx.next_upstream_peer() {
            UpstreamPeerResult::Addr(addr) => {
                ctx.instrumentation().record_upstream_peer(addr);
                Ok(Box::new(HttpPeer::new(addr, false, "".to_string())))
            }
            UpstreamPeerResult::NotFound => {
                ctx.instrumentation().record_status(StatusCode::NOT_FOUND);
                Err(Error::explain(
                    HTTPStatus(StatusCode::NOT_FOUND.into()),
                    "No matching route found",
                ))
            }
            UpstreamPeerResult::ServiceUnavailable => {
                ctx.instrumentation()
                    .record_status(StatusCode::SERVICE_UNAVAILABLE);
                Err(Error::explain(
                    HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                    "Service unavailable",
                ))
            }
            UpstreamPeerResult::MissingConfiguration => {
                ctx.instrumentation()
                    .record_status(StatusCode::SERVICE_UNAVAILABLE);
                Err(Error::explain(
                    HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                    "Missing configuration",
                ))
            }
        }
    }

    #[instrument(name = "request_filter", parent = ctx.instrumentation().request_span(), skip(self, session, ctx))]
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let client_addr_filter_handler_rx = self.client_addr_filter_handler_rx.clone();
        let client_addr =
            if let ReadyState::Ready(handler) = await_ready!(client_addr_filter_handler_rx) {
                handler.filter(session)
            } else {
                None
            };

        ctx.instrumentation().record_client_addr(client_addr);

        let router_rx = self.router_rx.clone();
        let route = if let ReadyState::Ready(router) = await_ready!(router_rx) {
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

        let access_control_filters_handlers_rx = self.access_control_filters_handlers_rx.clone();
        let error_code = match route {
            MatchRouteResult::Found(route, rule, matched_prefix) => {
                // if let ReadyState::Ready(handlers) =
                //     await_ready!(access_control_filters_handlers_rx)
                // {
                //     let handler = rule
                //         .filters()
                //         .iter()
                //         .filter_map(|f| f.ext_access_control.as_ref())
                //         .flat_map(|f| handlers.get(f.key()))
                //         .exactly_one();
                //
                //
                //
                //     match handler.evaluate(client_addr) {
                //         AccessControlEvaluationResult::Allowed => {
                //             debug!(
                //                 "Access control filters allowed request for client: {:?}",
                //                 client_addr
                //             );
                //         }
                //         AccessControlEvaluationResult::Denied => {
                //             info!(
                //                 "Access control filters denied request for client: {:?}",
                //                 client_addr
                //             );
                //             let response = ctx
                //                 .generate_error_response(ErrorResponseCode::AccessDenied)
                //                 .await;
                //
                //             let mut error_response = gen_error_response(response.status().into());
                //             self.set_response_server_header(&mut error_response)?;
                //             for (name, value) in response.headers() {
                //                 error_response.insert_header(name, value)?;
                //             }
                //
                //             session.write_response_header_ref(&error_response).await?;
                //             session
                //                 .write_response_body(response.body().clone(), true)
                //                 .await?;
                //             return Ok(true); // Request handled, don't proceed to upstream
                //         }
                //     }
                // } else {
                //     let response = ctx
                //         .generate_error_response(ErrorResponseCode::InvalidConfiguration)
                //         .await;
                //
                //     let mut error_response = gen_error_response(response.status().into());
                //     self.set_response_server_header(&mut error_response)?;
                //     for (name, value) in response.headers() {
                //         error_response.insert_header(name, value)?;
                //     }
                //
                //     session.write_response_header_ref(&error_response).await?;
                //     session
                //         .write_response_body(response.body().clone(), true)
                //         .await?;
                //     return Ok(true); // Request handled, don't proceed to upstream
                // }

                let static_responses_rx = self.static_responses_rx.clone();
                for filter in rule.filters() {
                    if let Some(ext_static_response) = &filter.ext_static_response
                        && let ReadyState::Ready(static_responses) =
                            await_ready!(static_responses_rx)
                        {
                            let static_filter = StaticResponseFilter::builder()
                                .responses(static_responses.clone())
                                .static_response_bodies(self.static_response_bodies_cache.clone())
                                .build();

                            match static_filter
                                .apply_to_session(session, ext_static_response.key())
                                .await
                            {
                                Ok(Some(status_code)) => {
                                    debug!(
                                        "Applied static response filter for route: {:?} with key: {}",
                                        route,
                                        ext_static_response.key()
                                    );
                                    ctx.instrumentation().record_status(status_code);
                                    return Ok(true);
                                }
                                Ok(None) => {
                                    debug!(
                                        "Static response key '{}' not found in configuration",
                                        ext_static_response.key()
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to apply static response filter for key '{}': {}",
                                        ext_static_response.key(),
                                        e
                                    );
                                }
                            }
                        }
                }

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
                            ctx.instrumentation()
                                .record_status(redirect_response.status_code);

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

                // Apply URL rewrite filters after redirect checks
                for filter in rule.filters() {
                    if let Some(url_rewrite) = &filter.url_rewrite {
                        let rewrite_filter = URLRewriteFilter::new(url_rewrite.clone());

                        // Create route match context with the matched prefix (reuse from redirect)
                        let route_context =
                            crate::proxy::filters::request_redirect::RouteMatchContext {
                                matched_prefix: matched_prefix.clone(),
                            };

                        // Apply URL rewrite to the request headers
                        if let Ok(was_rewritten) = rewrite_filter
                            .apply_to_pingora_request_with_context(
                                session.req_header_mut(),
                                &route_context,
                            )
                        {
                            if was_rewritten {
                                debug!(
                                    "Applied URL rewrite filter for route: {:?} with prefix: {:?}",
                                    route, matched_prefix
                                );
                            }
                        } else {
                            warn!("Failed to apply URL rewrite filter for route: {:?}", route);
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

        ctx.instrumentation().record_status(response.status());
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

    #[instrument(name = "early_request_filter", parent = ctx.instrumentation().request_span(), skip(self, session, ctx))]
    async fn early_request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        ctx.instrumentation().record_request(session.req_header());

        Ok(())
    }

    #[instrument(name = "upstream_request_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_request, ctx))]
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Apply backend-level header modifications from the matched route rule
        if let Some(MatchRouteResult::Found(route, rule, _)) = ctx.route() {
            if !rule.filters().is_empty() {
                for filter in rule.filters() {
                    if let Some(request_header_modifier) = &filter.request_header_modifier {
                        let header_filter =
                            RequestHeaderFilter::new(request_header_modifier.clone());
                        if let Err(e) = header_filter.apply_to_headers(upstream_request) {
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

        let mut upstream_req_headers = HeaderMap::new();
        ctx.instrumentation()
            .begin_upstream_call(&mut upstream_req_headers);
        for (header_name, header_value) in upstream_req_headers.iter() {
            let _ = upstream_request.insert_header(
                header_name.as_str().to_string(),
                header_value.to_str().unwrap_or("").to_string(),
            );
        }

        Ok(())
    }

    #[instrument(name = "upstream_response_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_response, ctx))]
    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        ctx.instrumentation().end_upstream_call(upstream_response);

        Ok(())
    }

    #[instrument(name = "response_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_response, ctx))]
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        ctx.instrumentation()
            .record_status(upstream_response.status);

        self.set_response_server_header(upstream_response)?;

        // Apply response header modifications from the matched route rule
        if let Some(context::MatchRouteResult::Found(route, rule, _)) = ctx.route() {
            if !rule.filters().is_empty() {
                for filter in rule.filters() {
                    if let Some(response_header_modifier) = &filter.response_header_modifier {
                        let header_filter =
                            ResponseHeaderFilter::new(response_header_modifier.clone());
                        if let Err(e) = header_filter.apply_to_headers(upstream_response) {
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
}

impl Proxy {
    fn set_response_server_header(&self, response: &mut ResponseHeader) -> Result<(), BError> {
        response.insert_header(SERVER, "Vale Gateway")?;
        Ok(())
    }
}
