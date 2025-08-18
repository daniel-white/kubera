use crate::proxy::responses::error_responses::{ErrorResponseCode, ErrorResponseGenerators};
use crate::proxy::router::endpoints::EndpointsResolver;
use crate::proxy::router::{HttpRoute, HttpRouteRule};
use bytes::Bytes;
use http::Response;
use opentelemetry::trace::Tracer;
use opentelemetry::{Context, ContextGuard};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;

#[derive(Debug)]
pub enum UpstreamPeerResult {
    Addr(SocketAddr),
    NotFound,
    ServiceUnavailable,
    MissingConfiguration,
}

#[derive(Debug)]
struct ContextState {
    route: MatchRouteResult,
    endpoint_resolver: Option<EndpointsResolver>,
    #[allow(dead_code)] // Future use for client IP tracking
    client_addr: Option<IpAddr>,
}

#[derive(TypedBuilder)]
pub struct RequestContext {
    #[builder(default)]
    tracing_context: OnceLock<Context>,

    error_response_generators_rx: Receiver<ErrorResponseGenerators>,

    #[builder(default)]
    state: OnceLock<ContextState>,

    #[builder(default)]
    otel_context: Option<opentelemetry::Context>,
}

unsafe impl Send for RequestContext {}

unsafe impl Sync for RequestContext {}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchRouteResult {
    Found(Arc<HttpRoute>, Arc<HttpRouteRule>, Option<String>), // Added matched_prefix
    NotFound,
    MissingConfiguration,
}

impl RequestContext {
    pub fn set_tracing_context(&mut self, context: Context) {
        let _ = self.tracing_context.get_or_init(|| context);
    }

    pub fn attach_tracing_context(&self) -> ContextGuard {
        let tracing_context = self.tracing_context.get().expect("Tracing context not set");
        tracing_context.clone().attach()
    }

    pub fn route(&self) -> Option<&MatchRouteResult> {
        self.state.get().map(|x| &x.route)
    }

    pub fn next_upstream_peer(&mut self) -> UpstreamPeerResult {
        if let Some(state) = self.state.get_mut()
            && let Some(resolver) = &mut state.endpoint_resolver
        {
            return if let Some(addr) = resolver.next() {
                UpstreamPeerResult::Addr(addr)
            } else {
                UpstreamPeerResult::NotFound
            };
        }
        match self.route() {
            Some(MatchRouteResult::NotFound) => UpstreamPeerResult::NotFound,
            Some(MatchRouteResult::MissingConfiguration) | None => {
                UpstreamPeerResult::MissingConfiguration
            }
            Some(MatchRouteResult::Found(_, _, _)) => UpstreamPeerResult::ServiceUnavailable,
        }
    }

    #[allow(dead_code)] // Public API for future client IP tracking
    pub fn client_addr(&self) -> Option<IpAddr> {
        self.state.get().and_then(|x| x.client_addr)
    }

    /// Get the current backend for header modification
    #[allow(dead_code)] // Public API for future backend context
    pub fn current_backend(&self) -> Option<&vg_core::config::gateway::types::net::Backend> {
        // For now, return None since the router uses different backend types
        // This will be updated when the router types are unified with core types
        None
    }

    pub fn set(&self, route: MatchRouteResult, client_addr: Option<IpAddr>) {
        let (route, endpoint_resolver) = match route {
            MatchRouteResult::Found(route, rule, matched_prefix) => {
                let mut resolver_builder = EndpointsResolver::builder(client_addr);
                resolver_builder.unique_id(rule.unique_id());
                for backend in rule.backends() {
                    for (location, endpoints) in backend.endpoints() {
                        for endpoint in endpoints {
                            resolver_builder.insert(endpoint.addr(), *location);
                        }
                    }
                }
                let endpoint_resolver = resolver_builder.build();
                (
                    MatchRouteResult::Found(route, rule, matched_prefix),
                    Some(endpoint_resolver),
                )
            }
            MatchRouteResult::NotFound => (MatchRouteResult::NotFound, None),
            MatchRouteResult::MissingConfiguration => {
                (MatchRouteResult::MissingConfiguration, None)
            }
        };

        let _ = self.state.set(ContextState {
            route,
            endpoint_resolver,
            client_addr,
        });
    }

    pub fn set_otel_context(&mut self, ctx: opentelemetry::Context) {
        self.otel_context = Some(ctx);
    }
    pub fn otel_context(&self) -> Option<&opentelemetry::Context> {
        self.otel_context.as_ref()
    }

    pub async fn generate_error_response(
        &self,
        code: ErrorResponseCode,
    ) -> Response<Option<Bytes>> {
        let generator = self
            .error_response_generators_rx
            .get()
            .await
            .unwrap_or_default();
        generator.get_response(code)
    }
}
