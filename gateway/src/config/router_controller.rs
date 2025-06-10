use crate::http::router::{Router, TransportSecurity, Upstream};
use http::{HeaderName, HeaderValue};

use kubera_core::config::gateway::types::HttpMethodMatch::Trace;
use kubera_core::config::gateway::types::{
    GatewayConfiguration, HostnameMatchType, HttpHeaderMatchType, HttpPathMatchType,
    HttpQueryParamMatchType,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ControllerError {}

pub async fn spawn_controller(
    gateway_configuration: Receiver<Option<GatewayConfiguration>>,
) -> Result<Receiver<Option<Router>>, ControllerError> {
    let mut gateway_configuration = gateway_configuration.clone();
    let (tx, rx) = channel(None);

    tokio::spawn(async move {
        loop {
            if let Some(gateway_config) = gateway_configuration.current() {
                let mut router_builder = Router::new_builder();
                for host in gateway_config.hosts().iter() {
                    for route in host.http_routes().iter() {
                        router_builder.route(|matcher_builder, upstreams_builder| {
                            for hostname in host.hostnames().iter() {
                                match hostname.match_type() {
                                    HostnameMatchType::Exact => {
                                        matcher_builder.with_host(hostname.value().get());
                                    }
                                    HostnameMatchType::Suffix => {
                                        matcher_builder.with_host_suffix(hostname.value().get());
                                    }
                                }
                            }

                            for item in route.matches().iter() {
                                for path in item.paths().iter() {
                                    match path.match_type() {
                                        HttpPathMatchType::Exact => {
                                            matcher_builder.with_exact_path(path.value())
                                        }
                                        HttpPathMatchType::Prefix => {
                                            matcher_builder.with_path_prefix(path.value())
                                        }
                                        HttpPathMatchType::RegularExpression => {
                                            matcher_builder.with_path_matching(path.value())
                                        }
                                    };
                                }

                                for method in item.methods().iter() {
                                    matcher_builder.with_method(method.clone().into());
                                }

                                for header in item.headers().iter() {
                                    match header.match_type() {
                                        HttpHeaderMatchType::Exact => matcher_builder.with_header(
                                            HeaderName::try_from(header.name()).unwrap(),
                                            HeaderValue::try_from(header.value()).unwrap(),
                                        ),
                                        HttpHeaderMatchType::RegularExpression => matcher_builder
                                            .with_header_matching(
                                                HeaderName::try_from(header.name()).unwrap(),
                                                header.value(),
                                            ),
                                    };
                                }

                                for query_param in item.query_params().iter() {
                                    match query_param.match_type() {
                                        HttpQueryParamMatchType::Exact => matcher_builder
                                            .with_query_param(
                                                query_param.name().get(),
                                                query_param.value(),
                                            ),
                                        HttpQueryParamMatchType::RegularExpression => {
                                            matcher_builder.with_query_param_matching(
                                                query_param.name().get(),
                                                query_param.value(),
                                            )
                                        }
                                    };
                                }
                            }

                            for backend in route.backends() {
                                debug!("Adding backend: {:?}", backend);
                                let upstream = match (
                                    backend.group().get().as_str(),
                                    backend.kind().get().as_str(),
                                ) {
                                    ("core", "Service") => Upstream::new_builder()
                                        .kubernetes_service(
                                            backend.namespace().get().clone(),
                                            backend.name().get().clone(),
                                            80,
                                        )
                                        .transport_security(TransportSecurity::None)
                                        .build()
                                        .inspect_err(|e| debug!("Failed to build upstream: {}", e))
                                        .ok(),
                                    _ => None,
                                };

                                if let Some(upstream) = upstream {
                                    upstreams_builder.add_upstream(upstream);
                                }
                            }
                        });
                    }
                }
                match router_builder.build() {
                    Ok(router) => {
                        tracing::info!("Router configuration updated");
                        tx.replace(Some(router));
                    }
                    Err(e) => {
                        tracing::error!("Failed to build router: {}", e);
                        tx.replace(None);
                    }
                }
            }

            select_continue!(gateway_configuration.changed())
        }
    });

    Ok(rx)
}
