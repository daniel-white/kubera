use tracing_opentelemetry::OpenTelemetrySpanExt;
mod constants;
mod context;
pub mod filters;
pub mod responses;
pub mod router;

use crate::controllers::static_response_bodies_cache::StaticResponseBodiesCache;
use crate::proxy::context::{MatchRouteResult, UpstreamPeerResult};
use crate::proxy::filters::downstream_request_context_injector::UpstreamRequestContextInjectorFilter;
use crate::proxy::filters::request_context_extractor::RequestContextExtractorFilter;
use crate::proxy::filters::static_responses::StaticResponseFilter;
use crate::proxy::responses::error_responses::{ErrorResponseCode, ErrorResponseGenerators};
use async_trait::async_trait;
use context::RequestContext;
use filters::client_addrs::ClientAddrFilter;
use filters::request_headers::RequestHeaderFilter;
use filters::request_redirect::RequestRedirectFilter;
use filters::response_headers::ResponseHeaderFilter;
use filters::url_rewrite::URLRewriteFilter;
use http::header::{HOST, SERVER, USER_AGENT};
use http::{HeaderMap, StatusCode};
use opentelemetry::global::get_text_map_propagator;
use opentelemetry::trace::Tracer;
use opentelemetry_semantic_conventions::attribute::URL_SCHEME;
use opentelemetry_semantic_conventions::trace::{
    CLIENT_ADDRESS, HTTP_RESPONSE_STATUS_CODE, NETWORK_PEER_ADDRESS, NETWORK_PEER_PORT,
    SERVER_ADDRESS, USER_AGENT_ORIGINAL,
};
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use router::HttpRouter;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info_span, instrument, warn, Span};
use typed_builder::TypedBuilder;
use vg_core::config::gateway::types::net::StaticResponse;
use vg_core::sync::signal::Receiver;

#[derive(TypedBuilder)]
pub struct Proxy {
    router_rx: Receiver<HttpRouter>,
    client_addr_filter_rx: Receiver<ClientAddrFilter>,
    error_responses_rx: Receiver<ErrorResponseGenerators>,
    static_responses_rx: Receiver<Arc<HashMap<String, StaticResponse>>>,
    static_response_bodies_cache: StaticResponseBodiesCache,
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        RequestContext::builder()
            .error_response_generators_rx(self.error_responses_rx.clone())
            .build()
    }

    #[instrument(name = "upstream_peer", parent = ctx.request_span(), skip(self, _session, ctx))]
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let span = Span::current();

        match ctx.next_upstream_peer() {
            UpstreamPeerResult::Addr(addr) => {
                span.set_attribute(NETWORK_PEER_ADDRESS, addr.ip().to_string());
                span.set_attribute(NETWORK_PEER_PORT, addr.port() as i64);
                Ok(Box::new(HttpPeer::new(addr, false, "".to_string())))
            }
            UpstreamPeerResult::NotFound => {
                let request_span = ctx.request_span();
                request_span.set_attribute(
                    HTTP_RESPONSE_STATUS_CODE,
                    format!("{:03}", StatusCode::NOT_FOUND.as_u16()),
                );
                Err(Error::explain(
                    HTTPStatus(StatusCode::NOT_FOUND.into()),
                    "No matching route found",
                ))
            }
            UpstreamPeerResult::ServiceUnavailable => {
                let request_span = ctx.request_span();
                request_span.set_attribute(
                    HTTP_RESPONSE_STATUS_CODE,
                    format!("{:03}", StatusCode::SERVICE_UNAVAILABLE.as_u16()),
                );
                Err(Error::explain(
                    HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                    "Service unavailable",
                ))
            }
            UpstreamPeerResult::MissingConfiguration => {
                let request_span = ctx.request_span();
                request_span.set_attribute(
                    HTTP_RESPONSE_STATUS_CODE,
                    format!("{:03}", StatusCode::SERVICE_UNAVAILABLE.as_u16()),
                );
                Err(Error::explain(
                    HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                    "Missing configuration",
                ))
            }
        }
    }

    #[instrument(name = "request_filter", parent = ctx.request_span(), skip(self, session, ctx))]
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let span = Span::current();
        let request_span = ctx.request_span();

        let client_addr = if let Some(client_addr_filter) = self.client_addr_filter_rx.get().await {
            client_addr_filter.filter(session)
        } else {
            None
        };

        if let Some(client_addr) = client_addr {
            request_span.set_attribute(CLIENT_ADDRESS, client_addr.to_string());
        }

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
                // Check for static response filters first - they should take precedence
                for filter in rule.filters() {
                    if let Some(ext_static_response) = &filter.ext_static_response {
                        if let Some(static_responses) = self.static_responses_rx.get().await {
                            let static_filter = StaticResponseFilter::builder()
                                .responses(static_responses)
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
                                    request_span.set_attribute(
                                        HTTP_RESPONSE_STATUS_CODE,
                                        format!("{:03}", status_code.as_u16()),
                                    );
                                    return Ok(true); // Request handled, don't proceed to upstream
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
                            request_span.set_attribute(
                                HTTP_RESPONSE_STATUS_CODE,
                                format!("{:03}", redirect_response.status_code.as_u16()),
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

    async fn early_request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let context = get_text_map_propagator(|p| {
            let filter = RequestContextExtractorFilter::new(p);
            filter.extract_from_headers(&session.req_header().headers)
        });

        let request = session.req_header();
        let http_method = request.method.as_str().to_ascii_uppercase();
        let uri = &request.uri;
        let path = uri.path().to_string();

        let span = info_span!("Request",
            otel.name = %http_method,
            otel.kind = "server",
            http.request.method = %http_method,
            url.path = %path,
        );

        span.set_parent(context);

        if let Some(scheme) = uri.scheme_str() {
            span.set_attribute(URL_SCHEME, scheme.to_ascii_lowercase());
        }

        if let Some(host) = request.headers.get(HOST)
            && let Some(host) = host.to_str().ok()
        {
            span.set_attribute(SERVER_ADDRESS, host.to_string());
        }

        if let Some(user_agent) = request.headers.get(USER_AGENT)
            && let Some(user_agent) = user_agent.to_str().ok()
        {
            span.set_attribute(USER_AGENT_ORIGINAL, user_agent.to_string());
        }

        ctx.set_request_span(span);

        Ok(())
    }

    #[instrument(name = "upstream_request_filter", parent = ctx.request_span(), skip(self, _session, upstream_request, ctx))]
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let request_span = ctx.request_span();

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

        let upstream_request_span = info_span!("upstream_request", otel.kind = "client");
        upstream_request_span.set_parent(request_span.context());

        get_text_map_propagator(|p| {
            let filter = UpstreamRequestContextInjectorFilter::new(p);
            let mut headers = HeaderMap::new();
            filter.apply_to_headers(&upstream_request_span, &mut headers);
            for (name, value) in headers {
                let _ = upstream_request.insert_header(name.expect("Unable to set header"), value);
            }
        });

        ctx.set_upstream_request_span(upstream_request_span);

        Ok(())
    }

    #[instrument(name = "upstream_response_filter", parent = ctx.request_span(), skip(self, _session, upstream_response, ctx))]
    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let upstream_response_span = ctx.upstream_request_span();

        upstream_response_span.set_attribute(
            HTTP_RESPONSE_STATUS_CODE,
            upstream_response.status.as_u16() as i64,
        );

        Ok(())
    }

    #[instrument(name = "response_filter", parent = ctx.request_span(), skip(self, _session, upstream_response, ctx))]
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let request_span = ctx.request_span();

        request_span.set_attribute(
            HTTP_RESPONSE_STATUS_CODE,
            format!("{:03}", upstream_response.status.as_u16()),
        );

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
