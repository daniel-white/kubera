use crate::http::route_matcher::RouteMatcher;
use derive_builder::Builder;
use http::HeaderValue;
use kubera_core::config::types::{
    GatewayConfiguration, HostnameMatchType, HttpHeaderMatchType,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use thiserror::Error;

#[derive(Default, Builder, Clone, PartialEq)]
pub struct Matchers {
    matchers: Vec<RouteMatcher>,
}

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("Failed to spawn controller")]
    SpawnError,
}

pub async fn spawn_controller(
    gateway_configuration: Receiver<Option<GatewayConfiguration>>,
) -> Result<Receiver<Matchers>, ControllerError> {
    let mut gateway_configuration = gateway_configuration.clone();
    let (tx, rx) = channel(Matchers::default());

    tokio::spawn(async move {
        loop {
            if let Some(config) = gateway_configuration.current() {
                let mut matchers = vec![];
                for host in config.hosts() {
                    for route in host.http_routes() {
                        let mut builder = RouteMatcher::new_builder();

                        for hostname in host.hostnames() {
                            match hostname.match_type() {
                                HostnameMatchType::Exact => {
                                    builder.with_hostname(hostname.value().get().clone());
                                }
                                HostnameMatchType::Suffix => {
                                    builder.with_hostname_suffix(hostname.value().get().clone());
                                }
                            }
                        }

                        for path in route.paths() {
                            builder.with_path(path.value().get().clone());
                        }

                        for item in route.matches() {
                            for header in item.headers() {
                                match header.type_() {
                                    HttpHeaderMatchType::Exact => {
                                        builder.with_header(
                                            header.name().into(),
                                            HeaderValue::from_str(header.value().as_str())
                                                .expect("Invalid header value"),
                                        );
                                    }
                                    HttpHeaderMatchType::RegularExpression => {
                                        builder.with_header_matching(
                                            header.name().into(),
                                            header.value().clone(),
                                        );
                                    }
                                }
                            }

                            builder.with_method(item.method().clone().into());
                        }

                        matchers.push(builder.build());
                    }
                }

                tx.replace(Matchers { matchers });
            }

            select_continue!(gateway_configuration.changed(),)
        }
    });

    Ok(rx)
}
