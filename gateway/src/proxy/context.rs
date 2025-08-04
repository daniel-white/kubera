use crate::proxy::responses::error_responses::{ErrorResponseCode, ErrorResponseGenerators};
use crate::proxy::router::endpoints::EndpointsResolver;
use crate::proxy::router::{HttpRoute, HttpRouteRule};
use bytes::Bytes;
use http::Response;
use kubera_core::sync::signal::Receiver;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use typed_builder::TypedBuilder;

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
    client_addr: Option<IpAddr>,
}

#[derive(TypedBuilder)]
pub struct Context {
    error_response_generators_rx: Receiver<ErrorResponseGenerators>,

    #[builder(default)]
    state: OnceLock<ContextState>,
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchRouteResult {
    Found(Arc<HttpRoute>, Arc<HttpRouteRule>),
    NotFound,
    MissingConfiguration,
}

impl Context {
    pub fn route(&self) -> Option<&MatchRouteResult> {
        self.state.get().map(|x| &x.route)
    }

    pub fn next_upstream_peer(&mut self) -> UpstreamPeerResult {
        if let Some(state) = self.state.get_mut() {
            if let Some(resolver) = &mut state.endpoint_resolver {
                return if let Some(addr) = resolver.next() {
                    UpstreamPeerResult::Addr(addr)
                } else {
                    UpstreamPeerResult::NotFound
                };
            }
        }
        match self.route() {
            Some(MatchRouteResult::NotFound) => UpstreamPeerResult::NotFound,
            Some(MatchRouteResult::MissingConfiguration) | None => {
                UpstreamPeerResult::MissingConfiguration
            }
            Some(MatchRouteResult::Found(_, _)) => UpstreamPeerResult::ServiceUnavailable,
        }
    }

    pub fn client_addr(&self) -> Option<IpAddr> {
        self.state.get().and_then(|x| x.client_addr)
    }

    pub fn set(&self, route: MatchRouteResult, client_addr: Option<IpAddr>) {
        let (route, endpoint_resolver) = match route {
            MatchRouteResult::Found(route, rule) => {
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
                    MatchRouteResult::Found(route, rule),
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
