use crate::proxy::router::topology::TopologyLocation;
use crate::proxy::router::{HttpRouter, HttpRouterBuilder};
use http::HeaderValue;
use std::sync::Arc;
use tracing::debug;
use vg_core::config::gateway::types::http::router::*;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn synthesize_http_router(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
    current_location: TopologyLocation,
) -> Receiver<HttpRouter> {
    let (tx, rx) = signal("http_router");

    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(synthesize_http_router))
        .spawn(async move {
            let current_location = Arc::new(current_location);
            loop {
                if let ReadyState::Ready(gateway_configuration) =
                    await_ready!(gateway_configuration_rx)
                {
                    let router = build_router(gateway_configuration, current_location.clone());
                    tx.set(router).await;
                }
                continue_on!(gateway_configuration_rx.changed())
            }
        });

    rx
}

fn build_router(
    gateway_config: &GatewayConfiguration,
    current_location: Arc<TopologyLocation>,
) -> HttpRouter {
    let mut router = HttpRouterBuilder::new(current_location);

    // for host_match in gateway_config.hosts().iter() {
    //     match hostMatch.match_type() {
    //         HostnameMatchType::Exact => {
    //             router.add_exact_host(hostMatch.value());
    //         }
    //         HostnameMatchType::Suffix => {
    //             router.add_host_suffix(hostMatch.value());
    //         }
    //     }
    // }

    for config_route in gateway_config.http_routes() {
        router.add_route(|route| {
            for host_header_match in config_route.host_header_matches() {
                match host_header_match.match_type() {
                    HostHeaderMatchType::Exact => {
                        route.add_exact_host(host_header_match.value());
                    }
                    HostHeaderMatchType::Suffix => {
                        route.add_host_suffix(host_header_match.value());
                    }
                }
            }

            for config_rule in config_route.rules() {
                route.add_rule(config_rule.unique_id().into(), |rule| {
                    for config_matches in config_rule.matches() {
                        rule.add_matches(|matches| {
                            let path = config_matches.path();
                            match (path.match_type(), path.value()) {
                                (HttpPathMatchType::Exact, value) => {
                                    matches.with_exact_path(value);
                                }
                                (HttpPathMatchType::Prefix, value) => {
                                    matches.with_path_prefix(value);
                                }
                                (HttpPathMatchType::RegularExpression, value) => {
                                    matches.with_path_matching(value);
                                }
                            }

                            if let Some(method) = config_matches.method() {
                                let method = *method;
                                matches.with_method(method.into());
                            }

                            if let Some(config_headers) = config_matches.headers() {
                                for config_header in config_headers.iter() {
                                    match config_header.match_type() {
                                        HttpHeaderMatchType::Exact => {
                                            matches.with_exact_header(
                                                config_header.name().try_into().unwrap(),
                                                HeaderValue::from_str(
                                                    config_header.value().as_str(),
                                                )
                                                .unwrap(),
                                            );
                                        }
                                        HttpHeaderMatchType::RegularExpression => {
                                            matches.with_header_matching(
                                                config_header.name().try_into().unwrap(),
                                                config_header.value().as_str(),
                                            );
                                        }
                                    }
                                }
                            }

                            if let Some(config_query_params) = config_matches.query_params() {
                                for config_query_param in config_query_params.iter() {
                                    match config_query_param.match_type() {
                                        HttpQueryParamMatchType::Exact => {
                                            matches.with_exact_query_param(
                                                config_query_param.name().get().as_str(),
                                                config_query_param.value().as_str(),
                                            );
                                        }
                                        HttpQueryParamMatchType::RegularExpression => {
                                            matches.with_query_param_matching(
                                                config_query_param.name().get().as_str(),
                                                config_query_param.value().as_str(),
                                            );
                                        }
                                    }
                                }
                            }
                        });
                    }

                    // Add filters from config rule to runtime rule
                    for config_filter in config_rule.filters() {
                        rule.add_filter(config_filter.clone());
                    }

                    for config_backend in config_rule.backends() {
                        rule.add_backend(|backend| {
                            if let Some(weight) = config_backend.weight() {
                                backend.with_weight(*weight);
                            }

                            if let Some(port) = config_backend.port() {
                                backend.with_port(*port.get());
                            }

                            for config_endpoint in config_backend.endpoints() {
                                let location = TopologyLocation::builder()
                                    .zone(config_endpoint.zone().clone())
                                    .node(config_endpoint.node().clone())
                                    .build();
                                backend.add_endpoint(*config_endpoint.address(), location);
                            }
                        });
                    }
                });
            }
        });
    }

    router.build()
}

#[cfg(test)]
mod tests {
    use crate::controllers::router::build_router;
    use crate::proxy::router::topology::TopologyLocation;
    use http::request::Builder;
    use std::io::Cursor;
    use std::sync::Arc;
    use vg_core::config::gateway::serde::read_configuration;

    #[test]
    fn test_router_simple() {
        let config = include_str!("./testcases/simple.yaml").to_string();
        let config = read_configuration(Cursor::new(config)).expect("Failed to read configuration");

        let current_location = TopologyLocation::builder()
            .zone(Some("zone1".to_string()))
            .node(Some("node1".to_string()));
        let current_location = current_location.build();
        let current_location = Arc::new(current_location);

        let router = build_router(&config, current_location);

        // Test with the root path "/" which matches the configuration
        let req = Builder::default().method("GET").uri("/").body(()).unwrap();

        let (parts, _) = req.into_parts();

        // Since there are no host matches defined in the config, the router should accept any host
        // or no host at all. The path "/" should match the prefix "/" rule in the config.
        router.match_route(&parts).expect("Failed to match route");
    }
}
