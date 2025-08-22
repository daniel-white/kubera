use crate::proxy::filters::client_addrs::extractors::ClientAddrExtractorType::{
    Noop, TrustedHeader, TrustedProxies,
};
use crate::proxy::filters::client_addrs::extractors::{
    TrustedHeaderClientAddrExtractor, TrustedProxiesClientAddrExtractor,
};
use crate::proxy::filters::client_addrs::ClientAddrFilterHandler;
use http::HeaderName;
use std::str::FromStr;
use std::sync::Arc;
use vg_core::config::gateway::types::net::{ClientAddrsSource, ProxyHeaders};
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn client_addr_filter_handler(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<ClientAddrFilterHandler> {
    let (tx, rx) = signal(stringify!(client_addr_filter));
    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(client_addr_filter))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway_configuration) =
                    await_ready!(gateway_configuration_rx)
                {
                    let extractor = if let Some(client_addrs) = gateway_configuration.client_addrs()
                    {
                        match client_addrs.source() {
                            ClientAddrsSource::None => Noop,
                            ClientAddrsSource::Header => client_addrs
                                .header()
                                .as_ref()
                                .and_then(|h| HeaderName::from_str(h).ok())
                                .map(|h| {
                                    let extractor = TrustedHeaderClientAddrExtractor::builder()
                                        .header(h)
                                        .build();
                                    TrustedHeader(Arc::new(extractor))
                                })
                                .unwrap_or(Noop),
                            ClientAddrsSource::Proxies => {
                                if let Some(proxies) = client_addrs.proxies().as_ref() {
                                    let mut extractor_builder =
                                        TrustedProxiesClientAddrExtractor::builder();

                                    for ip in proxies.trusted_ips().iter().cloned() {
                                        extractor_builder.add_trusted_ip(ip);
                                    }

                                    for range in proxies.trusted_ranges().iter().cloned() {
                                        extractor_builder.add_trusted_ip_range(range);
                                    }

                                    for header in proxies.trusted_headers() {
                                        match header {
                                            ProxyHeaders::Forwarded => {
                                                extractor_builder.trust_forwarded_header();
                                            }
                                            ProxyHeaders::XForwardedFor => {
                                                extractor_builder.trust_x_forwarded_for_header();
                                            }
                                            ProxyHeaders::XForwardedBy => {
                                                extractor_builder.trust_x_forwarded_by_header();
                                            }
                                            ProxyHeaders::XForwardedProto => {
                                                extractor_builder.trust_x_forwarded_proto_header();
                                            }
                                            ProxyHeaders::XForwardedHost => {
                                                extractor_builder.trust_x_forwarded_host_header();
                                            }
                                        }
                                    }

                                    TrustedProxies(Arc::new(extractor_builder.build()))
                                } else {
                                    Noop
                                }
                            }
                        }
                    } else {
                        Noop
                    };
                    let handler = ClientAddrFilterHandler::builder()
                        .extractor(extractor)
                        .build();
                    tx.set(handler).await;
                }
                continue_on!(gateway_configuration_rx.changed());
            }
        });

    rx
}
