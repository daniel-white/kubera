use crate::http::router::{Route, RouteMatcher, Router, RouterBuilder};
use http::{HeaderName, HeaderValue, Method};

use kubera_core::config::gateway::types::{
    GatewayConfiguration, HostnameMatchType, HttpHeaderMatchType, HttpHeaderName,
    HttpPathMatchType, HttpQueryParamMatchType,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("Failed to spawn controller")]
    SpawnError,
}

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
                        router_builder.route(|b| {
                            for hostname in host.hostnames().iter() {
                                match hostname.match_type() {
                                    HostnameMatchType::Exact => {
                                        b.with_host(hostname.value().get());
                                    }
                                    HostnameMatchType::Suffix => {
                                        b.with_host_suffix(hostname.value().get());
                                    }
                                }
                            }

                            for item in route.matches().iter() {
                                for path in item.paths().iter() {
                                    match path.match_type() {
                                        HttpPathMatchType::Exact => b.with_exact_path(path.value()),
                                        HttpPathMatchType::Prefix => {
                                            b.with_path_prefix(path.value())
                                        }
                                        HttpPathMatchType::RegularExpression => {
                                            b.with_path_matching(path.value())
                                        }
                                    };
                                }

                                for method in item.methods().iter() {
                                    b.with_method(method.clone().into());
                                }

                                for header in item.headers().iter() {
                                    match header.match_type() {
                                        HttpHeaderMatchType::Exact => b.with_header(
                                            HeaderName::try_from(header.name()).unwrap(),
                                            HeaderValue::try_from(header.value()).unwrap(),
                                        ),
                                        HttpHeaderMatchType::RegularExpression => b
                                            .with_header_matching(
                                                HeaderName::try_from(header.name()).unwrap(),
                                                header.value(),
                                            ),
                                    };
                                }

                                for query_param in item.query_params().iter() {
                                    match query_param.match_type() {
                                        HttpQueryParamMatchType::Exact => b.with_query_param(
                                            query_param.name().get(),
                                            query_param.value(),
                                        ),
                                        HttpQueryParamMatchType::RegularExpression => b
                                            .with_query_param_matching(
                                                query_param.name().get(),
                                                query_param.value(),
                                            ),
                                    };
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
