use tracing_opentelemetry::OpenTelemetrySpanExt;
mod constants;
mod context;
pub mod filters;
pub mod responses;
pub mod router;

use crate::controllers::static_response_bodies_cache::StaticResponseBodiesCache;
use crate::instrumentation::TRACER;
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
use futures::TryStreamExt;
use http::header::SERVER;
use http::request::Parts;
use http::StatusCode;
use opentelemetry::global::get_text_map_propagator;
use opentelemetry::trace::{SamplingResult, SpanKind, Status, TraceContextExt, TraceState, Tracer};
use opentelemetry::KeyValue;
use opentelemetry_semantic_conventions::attribute::{HTTP_REQUEST_METHOD, URL_PATH, URL_SCHEME};
use opentelemetry_semantic_conventions::trace::{
    HTTP_RESPONSE_STATUS_CODE, NETWORK_PEER_ADDRESS, NETWORK_PEER_PORT,
};
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use router::HttpRouter;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tracing::field::Visit;
use tracing::{debug, info_span, warn, Instrument, Span};
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

    async fn early_request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let request = session.req_header();
        let http_method = request.method.as_str();
        let uri = &request.uri;

        let mut attributes = vec![
            KeyValue::new(HTTP_REQUEST_METHOD, http_method.to_ascii_uppercase()),
            KeyValue::new(URL_PATH, uri.path().to_string()),
        ];

        if let Some(scheme) = uri.scheme_str() {
            attributes.push(KeyValue::new(URL_SCHEME, scheme.to_ascii_lowercase()));
        }

        let context = get_text_map_propagator(|p| {
            let filter = RequestContextExtractorFilter::new(p);
            filter.extract_from_headers(&session.req_header().headers)
        });

        let span = TRACER
            .span_builder(http_method.to_ascii_uppercase())
            .with_kind(SpanKind::Server)
            .with_attributes(attributes)
            .with_sampling_result(SamplingResult {
                decision: opentelemetry::trace::SamplingDecision::RecordAndSample,
                attributes: vec![],
                trace_state: TraceState::NONE,
            })
            .start_with_context(&*TRACER, &context);

        ctx.set_tracing_context(context);

        Ok(())
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let _ = ctx.attach_tracing_context();
        let span = Span::current();

        async {
            match ctx.next_upstream_peer() {
                UpstreamPeerResult::Addr(addr) => {
                    let span = Span::current();
                    span.set_attribute(NETWORK_PEER_ADDRESS, addr.ip().to_string());
                    span.set_attribute(NETWORK_PEER_PORT, addr.port() as i64);
                    Ok(Box::new(HttpPeer::new(addr, false, "".to_string())))
                }
                UpstreamPeerResult::NotFound => {
                    span.set_attribute(
                        HTTP_RESPONSE_STATUS_CODE,
                        format!("{:03}", StatusCode::NOT_FOUND.as_u16()),
                    );
                    Err(Error::explain(
                        HTTPStatus(StatusCode::NOT_FOUND.into()),
                        "No matching route found",
                    ))
                }
                UpstreamPeerResult::ServiceUnavailable => {
                    span.set_attribute(
                        HTTP_RESPONSE_STATUS_CODE,
                        format!("{:03}", StatusCode::SERVICE_UNAVAILABLE.as_u16()),
                    );
                    Err(Error::explain(
                        HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                        "Service unavailable",
                    ))
                }
                UpstreamPeerResult::MissingConfiguration => {
                    span.set_attribute(
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
        .instrument(info_span!("upstream_peer"))
        .await
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let _ = ctx.attach_tracing_context();
        let span = Span::current();

        async move {
            let span = Span::current();
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
                                        span.set_attribute(HTTP_RESPONSE_STATUS_CODE, format!("{:03}", status_code.as_u16()));
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
                            } else {
                                warn!("No static responses configuration available");
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
                                span.set_attribute(HTTP_RESPONSE_STATUS_CODE, format!("{:03}", redirect_response.status_code.as_u16()));

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
        }.instrument(info_span!("request_filter")).await
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let _ = ctx.attach_tracing_context();

        info_span!("upstream_request_filter").in_scope(|| {
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

            // Apply tracing after route configuration.
            get_text_map_propagator(|p| {
                let filter = UpstreamRequestContextInjectorFilter::new(p);
                let mut headers = upstream_request.as_owned_parts().headers;
                filter.apply_to_headers(&mut headers);
            });
            Ok(())
        })
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let _ = ctx.attach_tracing_context();

        let span = Span::current();
        span.set_attribute(
            HTTP_RESPONSE_STATUS_CODE,
            format!("{:03}", upstream_response.status.as_u16()),
        );

        async {
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
        .instrument(info_span!("response_filter"))
        .await
    }
}

impl Proxy {
    fn set_response_server_header(&self, response: &mut ResponseHeader) -> Result<(), BError> {
        response.insert_header(SERVER, "Vale Gateway")?;
        Ok(())
    }
}
