use std::ops::Deref;
use std::sync::Arc;
use crate::proxy::router::topology::TopologyLocation;
use crate::proxy::router::{HttpRouter, HttpRouterBuilder};
use http::HeaderValue;
use tracing::debug;
use vg_core::http::listeners::HttpListener;
use vg_core::http::matches::{
    HttpHeaderMatchKind, HttpHostHeaderMatchKind, HttpPathMatchKind, HttpQueryParamMatchKind,
};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_router(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
    current_location: TopologyLocation,
) -> Receiver<Option<HttpRouter>> {
    let (tx, rx) = signal(stringify!(http_router));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_router))
        .spawn(async move {
            let current_location = Arc::new(current_location);
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let http_router =
                        http_listener.as_ref().map(|l| build_http_router(l, current_location.clone()));
                    tx.set(http_router).await;
                }
                continue_on!(http_listener_rx.changed())
            }
        });

    rx
}

fn build_http_router(
    http_listener: &HttpListener,
    current_location: Arc<TopologyLocation>,
) -> HttpRouter {
    let mut builder = HttpRouterBuilder::new(current_location);

    // TODO add filter support

    for route in http_listener.routes() {
        builder.add_route(|builder| {
            for host_header_match in route.host_header_matches() {
                match host_header_match.kind() {
                    HttpHostHeaderMatchKind::Exact => {
                        builder.add_exact_host(host_header_match.value().as_str());
                    }
                    HttpHostHeaderMatchKind::Suffix => {
                        builder.add_host_suffix(host_header_match.value().as_str());
                    }
                }
            }

            for rule in route.rules() {
                builder.add_rule(rule.key().into(), |builder| {
                    for r#match in rule.matches() {
                        builder.add_matches(|builder| {
                            let path = r#match.path();
                            match (path.kind(), path.value()) {
                                (HttpPathMatchKind::Exact, value) => {
                                    builder.with_exact_path(value);
                                }
                                (HttpPathMatchKind::Prefix, value) => {
                                    builder.with_path_prefix(value);
                                }
                                (HttpPathMatchKind::RegularExpression, value) => {
                                    builder.with_path_matching(value);
                                }
                            }

                            if let Some(method) = r#match.method().as_ref() {
                                builder.with_method(method.into());
                            }

                            if let Some(headers) = r#match.headers() {
                                for header in headers.iter() {
                                    match header.kind() {
                                        HttpHeaderMatchKind::Exact => {
                                            builder
                                                .with_exact_header(header.header(), HeaderValue::from_str(header.value()).unwrap());
                                        }
                                        HttpHeaderMatchKind::RegularExpression => {
                                            builder.with_header_matching(
                                                header.header(),
                                                header.value(),
                                            );
                                        }
                                    }
                                }
                            }

                            if let Some(query_params) = r#match.query_params() {
                                for query_param in query_params.iter() {
                                    match query_param.kind() {
                                        HttpQueryParamMatchKind::Exact => {
                                            builder.with_exact_query_param(
                                                query_param.name().as_str(),
                                                query_param.value().as_str(),
                                            );
                                        }
                                        HttpQueryParamMatchKind::RegularExpression => {
                                            builder.with_query_param_matching(
                                                query_param.name().as_str(),
                                                query_param.value().as_str(),
                                            );
                                        }
                                    }
                                }
                            }
                        });
                    }

                    // Add filters from config rule to runtime rule
                    for filter in rule.filters() {
                        builder.add_filter(filter.clone());
                    }

                    for backend in rule.backends() {
                        builder.add_backend(|builder| {
                            if let Some(weight) = backend.weight() {
                                builder.with_weight(*weight);
                            }

                            for endpoint in backend.endpoints() {
                                let location = TopologyLocation::builder()
                                    .zone(endpoint.zone().clone())
                                    .node(endpoint.node().clone())
                                    .build();
                                builder.add_endpoint(*endpoint.addr(), location);
                            }
                        });
                    }
                });
            }
        });
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::request::Builder;
    use vg_core::http::matches::HttpMethodMatch;

    #[test]
    fn test_router_simple() {
        let mut http_listener = HttpListener::builder();
        http_listener
            .name("listener1".to_string())
            .add_route("route1", |builder| {
                builder.add_host_header_match(|builder| {
                    builder.exactly("example.com");
                });
                builder.add_rule("rule1", |builder| {
                    builder.add_match(|builder| {
                        builder.with_path_prefix("/metrics");
                        builder.with_method(HttpMethodMatch::Get);
                    });
                });
            });
        let http_listener = http_listener.build();

        let current_location = TopologyLocation::builder()
            .zone(Some("zone1".to_string()))
            .node(Some("node1".to_string()));
        let current_location = current_location.build();

        let router = build_http_router(&http_listener, Arc::new(current_location));

        // Test with the root path "/" which matches the configuration
        let req = Builder::default().method("GET").uri("/").body(()).unwrap();

        let (parts, _) = req.into_parts();

        // Since there are no host matches defined in the config, the router should accept any host
        // or no host at all. The path "/" should match the prefix "/" rule in the config.
        router.match_route(&parts).expect("Failed to match route");
    }
}
