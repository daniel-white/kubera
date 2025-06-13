use crate::http::router::HttpRouter;
use http::{HeaderName, HeaderValue};

use kubera_core::config::gateway::types::http::router::*;
use kubera_core::config::gateway::types::net::HostMatchType;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ControllerError {}

pub async fn spawn_controller(
    gateway_configuration: Receiver<Option<GatewayConfiguration>>,
) -> Result<Receiver<Option<HttpRouter>>, ControllerError> {
    let mut gateway_configuration = gateway_configuration.clone();
    let (tx, rx) = channel(None);

    tokio::spawn(async move {
        loop {
            if let Some(gateway_config) = gateway_configuration.current().as_ref() {
                let mut router = HttpRouter::new_builder();

                for host_match in gateway_config.hosts().iter() {
                    match host_match.match_type() {
                        HostMatchType::Exact => {
                            router.add_exact_host(host_match.value().get());
                        }
                        HostMatchType::Suffix => {
                            router.add_host_suffix(host_match.value().get());
                        }
                    }
                }

                for config_route in gateway_config.http_routes() {
                    router.add_route(|route| {
                        for host_header_match in config_route.host_header_matches() {
                            match host_header_match.match_type() {
                                HostHeaderMatchType::Exact => {
                                    route.add_exact_host(host_header_match.value().get());
                                }
                                HostHeaderMatchType::Suffix => {
                                    route.add_host_suffix(host_header_match.value().get());
                                }
                            }
                        }

                        for config_rule in config_route.rules() {
                            route.add_rule(|rule| {
                                for config_matches in config_rule.matches() {
                                    rule.add_matches(|matches| {
                                        match config_matches
                                            .path()
                                            .as_ref()
                                            .map(|p| (p.match_type(), p.value()))
                                        {
                                            Some((HttpPathMatchType::Exact, value)) => {
                                                matches.with_exact_path(value);
                                            }
                                            Some((HttpPathMatchType::Prefix, value)) => {
                                                matches.with_path_prefix(value);
                                            }
                                            Some((HttpPathMatchType::RegularExpression, value)) => {
                                                matches.with_path_matching(value);
                                            }
                                            _ => (),
                                        }

                                        if let Some(config_method) = config_matches.method() {
                                            matches.with_method(config_method.clone().into());
                                        }

                                        if let Some(config_headers) = config_matches.headers() {
                                            for config_header in config_headers.iter() {
                                                match config_header.match_type() {
                                                    HttpHeaderMatchType::Exact => {
                                                        matches.with_exact_header(
                                                            config_header.name().into(),
                                                            HeaderValue::from_str(
                                                                config_header.value().as_str(),
                                                            )
                                                            .unwrap(),
                                                        );
                                                    }
                                                    HttpHeaderMatchType::RegularExpression => {
                                                        matches.with_header_matching(
                                                            config_header.name().into(),
                                                            config_header.value().as_str(),
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(config_query_params) =
                                            config_matches.query_params()
                                        {
                                            for config_query_param in config_query_params.iter() {
                                                match config_query_param.match_type() {
                                                    HttpQueryParamMatchType::Exact => {
                                                        matches.with_exact_query_param(
                                                            config_query_param
                                                                .name()
                                                                .get()
                                                                .as_str(),
                                                            config_query_param.value().as_str(),
                                                        );
                                                    }
                                                    HttpQueryParamMatchType::RegularExpression => {
                                                        matches.with_query_param_matching(
                                                            config_query_param
                                                                .name()
                                                                .get()
                                                                .as_str(),
                                                            config_query_param.value().as_str(),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }

                                for config_backend in config_rule.backends() {
                                    rule.add_backend(|backend| {
                                        backend.with_weight(*config_backend.weight());

                                        if let Some(port) = config_backend.port() {
                                            backend.with_port(*port.get());
                                        }

                                        for config_endpoint in config_backend.endpoints() {
                                            backend.add_endpoint(
                                                *config_endpoint.address(),
                                                |endpoint| {
                                                    if let Some(zone) = config_endpoint.zone() {
                                                        endpoint.in_zone(zone.clone());
                                                    }
                                                    if let Some(node) = config_endpoint.node() {
                                                        endpoint.on_node(node.clone());
                                                    }
                                                },
                                            );
                                        }
                                    });
                                }
                            });
                        }
                    });
                }
                let router = router.build();
                tx.replace(Some(router));
            }

            select_continue!(gateway_configuration.changed())
        }
    });

    Ok(rx)
}
